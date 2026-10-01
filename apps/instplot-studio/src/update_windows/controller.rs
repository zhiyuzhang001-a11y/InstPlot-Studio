//! Pollable helper engine. No UI entry point, background thread, or forced exit.
//! The caller must first finish work protection and protocol compatibility.
//! Keep this object alive on inspection/error: it owns native children/leases.
use crate::update_transaction::UpdateStage;
use std::io;
use std::time::{Duration, Instant};

use super::{
    RunningWindowsInstaller, WindowsCandidateLaunch, WindowsCandidateProcess, WindowsHelperSession,
    WindowsInstallAccess, invalid,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsControllerPhase {
    WaitingForExit,
    Applying,
    AwaitingCandidate,
    StoppingCandidate,
    Restoring,
    AwaitingRecovery,
    StoppingRecovery,
    Completed,
    RolledBack,
    FailedBeforeApply,
    InspectionRequired,
}

pub struct WindowsUpdateController<'a> {
    helper: &'a WindowsHelperSession,
    phase: WindowsControllerPhase,
    installer: Option<RunningWindowsInstaller<'a>>,
    gui: Option<WindowsCandidateProcess<'a>>,
    access: Option<WindowsInstallAccess>,
    entered: Instant,
    exit_recorded: bool,
}

impl<'a> WindowsUpdateController<'a> {
    /// Called only in the authenticated copied helper after parent-side work
    /// and compatibility preflight. Readiness is durable before the parent is
    /// allowed to close. This method does not close the parent or install.
    pub fn wait_after_preflight(helper: &'a WindowsHelperSession) -> io::Result<Self> {
        helper.acknowledge_ready()?;
        Ok(Self {
            helper,
            phase: WindowsControllerPhase::WaitingForExit,
            installer: None,
            gui: None,
            access: None,
            entered: Instant::now(),
            exit_recorded: false,
        })
    }

    /// Internal adapter after work freezing, compatibility checks, helper
    /// readiness and old-GUI normal exit. NOT authorization to bypass those
    /// gates. There is deliberately no production caller/CLI yet.
    pub fn start_after_preflight(
        helper: &'a WindowsHelperSession,
        access: WindowsInstallAccess,
    ) -> io::Result<Self> {
        let installer = helper.start_candidate_installer(access)?;
        Ok(Self {
            helper,
            phase: WindowsControllerPhase::Applying,
            installer: Some(installer),
            gui: None,
            access: None,
            entered: Instant::now(),
            exit_recorded: false,
        })
    }

    pub fn phase(&self) -> WindowsControllerPhase {
        self.phase
    }

    /// One nonblocking step. Errors latch inspection and retain all owned
    /// witnesses; another poll cannot replay installation, launch, or recovery.
    pub fn poll(&mut self) -> io::Result<WindowsControllerPhase> {
        if self.phase == WindowsControllerPhase::InspectionRequired {
            return Err(invalid(
                "update inspection required; native witnesses retained",
            ));
        }
        if let Err(error) = self.step() {
            self.phase = WindowsControllerPhase::InspectionRequired;
            return Err(error);
        }
        Ok(self.phase)
    }

    fn enter(&mut self, phase: WindowsControllerPhase) {
        self.phase = phase;
        self.entered = Instant::now();
    }

    fn step(&mut self) -> io::Result<()> {
        match self.phase {
            WindowsControllerPhase::WaitingForExit => self.poll_old_exit(),
            WindowsControllerPhase::Applying | WindowsControllerPhase::Restoring => {
                self.poll_installer()
            }
            WindowsControllerPhase::AwaitingCandidate
            | WindowsControllerPhase::AwaitingRecovery => self.poll_health(),
            WindowsControllerPhase::StoppingCandidate
            | WindowsControllerPhase::StoppingRecovery => self.poll_normal_exit(),
            WindowsControllerPhase::Completed
            | WindowsControllerPhase::RolledBack
            | WindowsControllerPhase::FailedBeforeApply => Ok(()),
            WindowsControllerPhase::InspectionRequired => Err(invalid("inspection required")),
        }
    }

    fn poll_old_exit(&mut self) -> io::Result<()> {
        if exit_wait_expired(self.entered.elapsed()) {
            self.helper.abort_before_apply(
                "normal exit or installation exclusion timed out before apply",
            )?;
            self.enter(WindowsControllerPhase::FailedBeforeApply);
            return Ok(());
        }
        let exited = self.helper.old_process().wait_for_exit(Duration::ZERO)?;
        if !exited {
            return Ok(());
        }
        match WindowsInstallAccess::exclusive(self.helper.installation().directory()) {
            Ok(access) => self.access = Some(access),
            Err(_) => return Ok(()),
        }
        let installer = self.helper.start_candidate_installer(
            self.access
                .take()
                .ok_or_else(|| invalid("exclusion missing"))?,
        )?;
        self.installer = Some(installer);
        self.enter(WindowsControllerPhase::Applying);
        Ok(())
    }

    fn poll_installer(&mut self) -> io::Result<()> {
        let installer = self
            .installer
            .as_mut()
            .ok_or_else(|| invalid("installer witness missing"))?;
        let Some(status) = installer.try_wait()? else {
            // No production timeout authorizes killing a still-running installer.
            return Ok(());
        };
        self.access = Some(installer.take_owned_access_after_exit()?);
        let restoring = self.phase == WindowsControllerPhase::Restoring;
        if !status.success() {
            if restoring {
                return Err(invalid("recovery installer failed; evidence retained"));
            }
            let recovery = self.helper.start_recovery_after_failed_installer(
                installer,
                self.access
                    .take()
                    .ok_or_else(|| invalid("exclusion missing"))?,
                "candidate installer failed before GUI launch",
            )?;
            self.installer = Some(recovery);
            self.enter(WindowsControllerPhase::Restoring);
            return Ok(());
        }
        let state = self
            .helper
            .store()
            .read(self.helper.transaction().identity())?;
        let access = self
            .access
            .as_ref()
            .ok_or_else(|| invalid("exclusion missing"))?;
        let launch = if restoring {
            WindowsCandidateLaunch::reserve_recovery(self.helper, &state, access)?
        } else {
            WindowsCandidateLaunch::reserve(
                self.helper.store(),
                &state,
                self.helper.installation(),
                &self.helper.installers().candidate,
                access,
                self.helper.resume_project(),
            )?
        };
        self.gui = Some(
            launch.spawn_once(
                self.access
                    .take()
                    .ok_or_else(|| invalid("exclusion missing"))?,
            )?,
        );
        self.exit_recorded = false;
        self.enter(if restoring {
            WindowsControllerPhase::AwaitingRecovery
        } else {
            WindowsControllerPhase::AwaitingCandidate
        });
        Ok(())
    }

    fn poll_health(&mut self) -> io::Result<()> {
        let recovery = self.phase == WindowsControllerPhase::AwaitingRecovery;
        let gui = self
            .gui
            .as_mut()
            .ok_or_else(|| invalid("GUI witness missing"))?;
        if gui.accept_health().is_ok() || gui.verify_committed_health().is_ok() {
            self.helper.release_project_after_health(gui)?;
            self.enter(if recovery {
                WindowsControllerPhase::RolledBack
            } else {
                WindowsControllerPhase::Completed
            });
            return Ok(());
        }
        let state = self
            .helper
            .store()
            .read(self.helper.transaction().identity())?;
        let exited = gui.owned_process_exited()?;
        if !recovery && state.stage() == UpdateStage::Applying && exited {
            // Known CreateProcess failure or an owned child that exited before
            // binding. The durable owned-exit gate still runs before recovery.
            gui.request_normal_exit()?;
            self.enter(WindowsControllerPhase::StoppingCandidate);
            return Ok(());
        }
        let may_wait = health_wait_allowed(recovery, state.stage(), self.entered.elapsed())?;
        if gui.checkpoint_error().is_some() && !exited {
            return Err(invalid(
                "GUI launch checkpoint unresolved; native witness retained",
            ));
        }
        // No receipt yet is normal. A bad/missing checkpoint remains protected
        // while the authenticated GUI gets its bounded startup interval.
        if !exited && may_wait {
            return Ok(());
        }
        // The GUI polls its private stop request and closes normally. No Kill.
        gui.request_normal_exit()?;
        self.enter(if recovery {
            WindowsControllerPhase::StoppingRecovery
        } else {
            WindowsControllerPhase::StoppingCandidate
        });
        Ok(())
    }

    fn poll_normal_exit(&mut self) -> io::Result<()> {
        let recovery = self.phase == WindowsControllerPhase::StoppingRecovery;
        let gui = self
            .gui
            .as_mut()
            .ok_or_else(|| invalid("GUI witness missing"))?;
        if !gui.owned_process_exited()? {
            if self.entered.elapsed() >= Duration::from_secs(30) {
                return Err(invalid(
                    "GUI did not exit normally; retained for inspection",
                ));
            }
            return Ok(());
        }
        if !self.exit_recorded {
            gui.release_binary_after_exit()?;
            self.exit_recorded = true;
        }
        if recovery {
            return Err(invalid(
                "recovered GUI failed health; no second recovery launch",
            ));
        }
        if self.access.is_none() {
            match WindowsInstallAccess::exclusive(self.helper.installation().directory()) {
                Ok(access) => self.access = Some(access),
                Err(_) if self.entered.elapsed() < Duration::from_secs(30) => {
                    // Another supported instance may still hold shared access.
                    // Retain the exited native witness; never bypass that lock.
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }
        let installer = self.helper.start_recovery_installer_after_candidate(
            gui,
            self.access
                .take()
                .ok_or_else(|| invalid("exclusion missing"))?,
            "candidate GUI failed health confirmation",
        )?;
        self.installer = Some(installer);
        self.enter(WindowsControllerPhase::Restoring);
        Ok(())
    }
}

fn health_wait_allowed(recovery: bool, stage: UpdateStage, elapsed: Duration) -> io::Result<bool> {
    let expected = if recovery {
        UpdateStage::Restoring
    } else {
        UpdateStage::AwaitingHealth
    };
    if stage != expected {
        // Includes an already committed state with unavailable/changed receipt
        // or dead native witness. Do NOT close GUI or initiate rollback then.
        return Err(invalid(
            "health stage changed; inspection instead of rollback",
        ));
    }
    Ok(elapsed < Duration::from_secs(60))
}

fn exit_wait_expired(elapsed: Duration) -> bool {
    elapsed >= Duration::from_secs(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_exit_wait_has_an_exact_boundary_not_a_forced_exit() {
        assert!(!exit_wait_expired(Duration::ZERO));
        assert!(!exit_wait_expired(Duration::from_secs(29)));
        assert!(exit_wait_expired(Duration::from_secs(30)));
        assert!(exit_wait_expired(Duration::from_secs(31)));
    }

    #[test]
    fn committed_or_foreign_stage_never_authorizes_a_stop_or_rollback() {
        for recovery in [false, true] {
            for stage in [
                UpdateStage::Prepared,
                UpdateStage::WaitingForExit,
                UpdateStage::Applying,
                UpdateStage::AwaitingHealth,
                UpdateStage::RecoveryRequired,
                UpdateStage::Restoring,
                UpdateStage::Completed,
                UpdateStage::RolledBack,
                UpdateStage::FailedBeforeApply,
            ] {
                let expected = if recovery {
                    UpdateStage::Restoring
                } else {
                    UpdateStage::AwaitingHealth
                };
                if stage != expected {
                    assert!(health_wait_allowed(recovery, stage, Duration::ZERO).is_err());
                    assert!(health_wait_allowed(recovery, stage, Duration::from_secs(90)).is_err());
                }
            }
        }
    }

    #[test]
    fn only_matching_uncommitted_health_role_has_a_bounded_startup_wait() {
        for (recovery, stage) in [
            (false, UpdateStage::AwaitingHealth),
            (true, UpdateStage::Restoring),
        ] {
            assert!(health_wait_allowed(recovery, stage, Duration::from_secs(59)).unwrap());
            assert!(!health_wait_allowed(recovery, stage, Duration::from_secs(60)).unwrap());
            assert!(!health_wait_allowed(recovery, stage, Duration::from_secs(90)).unwrap());
        }
    }
}
