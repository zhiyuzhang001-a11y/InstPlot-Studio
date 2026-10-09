//! Parent-side ownership only; this does not enable the GUI update entry.
use std::io;
use std::process::Child;
use std::time::{Duration, Instant};

use super::{PreparedWindowsHelper, WindowsInstallAccess};

const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// A start failure never obtained a Child. Retain this owner in the frozen GUI
/// until exact cancellation succeeds; then drop it before unfreezing work so
/// its saved-project lease is actually released. Failure keeps inspection.
#[must_use = "retain start failure resources until cancellation is verified"]
pub struct WindowsParentStartFailure {
    error: io::Error,
    prepared: PreparedWindowsHelper,
}

impl WindowsParentStartFailure {
    pub fn error(&self) -> &io::Error {
        &self.error
    }

    pub fn cancel_before_any_child(&self) -> io::Result<()> {
        self.prepared.cancel_unspawned_before_exit()
    }
}

impl std::fmt::Debug for WindowsParentStartFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WindowsParentStartFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for WindowsParentStartFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for WindowsParentStartFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsParentReadiness {
    Waiting,
    Ready,
}

struct OwnedHelper {
    prepared: PreparedWindowsHelper,
    child: Child,
}

/// Keep this owner in the frozen parent until normal GUI exit, or until
/// `cancel_and_poll_exit` returns true. Errors never authorize close/unfreeze.
/// This owner may move to a worker; it never kills processes or runs installers.
#[must_use = "retain the helper owner while the parent is frozen"]
pub struct WindowsParentHelper {
    owned: Option<OwnedHelper>,
    started: Instant,
    cancellation_requested: bool,
    last_ready: Option<Instant>,
}

impl WindowsParentHelper {
    /// Call only after user restart consent and successful work protection.
    /// A spawn error grants no permission to close the parent. The existing
    /// adapter persists cancellation or leaves inspection evidence on failure.
    pub fn start_after_work_protection(
        prepared: PreparedWindowsHelper,
    ) -> Result<Self, Box<WindowsParentStartFailure>> {
        let child = match prepared.spawn_after_work_protection() {
            Ok(child) => child,
            Err(error) => return Err(Box::new(WindowsParentStartFailure { error, prepared })),
        };
        // No fallible operation after acquiring the actual Child.
        Ok(Self {
            owned: Some(OwnedHelper { prepared, child }),
            started: Instant::now(),
            cancellation_requested: false,
            last_ready: None,
        })
    }

    /// Recheck immediately before requesting normal GUI exit, even if an
    /// earlier poll returned Ready. The owned process, nonce, durable state,
    /// original installation and files are authenticated on every success.
    pub fn poll_readiness(
        &mut self,
        parent_access: &WindowsInstallAccess,
    ) -> io::Result<WindowsParentReadiness> {
        if self.cancellation_requested {
            return Err(invalid(
                "readiness is disabled after cancellation was requested",
            ));
        }
        let owned = self
            .owned
            .as_mut()
            .ok_or_else(|| invalid("helper ownership has already ended"))?;
        if owned.child.try_wait()?.is_some() {
            return Err(invalid("owned helper exited before GUI close"));
        }
        parent_access.require_parent_exit_access(&owned.prepared)?;
        let proof = owned.prepared.confirm_ready(&owned.child);
        let readiness = readiness_result(self.started.elapsed(), proof)?;
        self.last_ready = (readiness == WindowsParentReadiness::Ready).then(Instant::now);
        Ok(readiness)
    }

    /// GUI's final cheap close gate after full background verification. The
    /// actual owned child must still be alive; a stale proof never closes GUI.
    pub fn confirm_recent_ready(&mut self) -> io::Result<()> {
        recent_ready_gate(
            self.cancellation_requested,
            self.last_ready.map(|when| when.elapsed()),
            || {
                let owned = self
                    .owned
                    .as_mut()
                    .ok_or_else(|| invalid("missing helper owner"))?;
                owned.child.try_wait().map(|status| status.is_some())
            },
        )
    }

    /// Promote the parent's original shared guard only after the helper's
    /// WaitingForExit request blocks new startup. Retain that exact guard until
    /// GUI exit, or restore shared access after completed cancellation. A failed
    /// promotion permanently disables readiness on this owner; cancel/inspect.
    pub fn promote_installation_access(
        &mut self,
        parent_access: &mut WindowsInstallAccess,
    ) -> io::Result<()> {
        if self.cancellation_requested {
            return Err(invalid("cannot promote after cancellation was requested"));
        }
        let owned = self
            .owned
            .as_ref()
            .ok_or_else(|| invalid("helper ownership has already ended"))?;
        let result = parent_access.promote_for_parent_exit(&owned.prepared);
        if result.is_err() {
            self.cancellation_requested = true;
        }
        result
    }

    /// Publish exact cancellation before observing exit. Keep work frozen on
    /// false OR error. Only true means the actual helper exited, cancellation
    /// journal was committed/revalidated afterwards, and leases were released.
    pub fn cancel_and_poll_exit(
        &mut self,
        parent_access: &mut WindowsInstallAccess,
    ) -> io::Result<bool> {
        self.cancellation_requested = true;
        let Some(owned) = self.owned.as_mut() else {
            return Ok(true);
        };
        let prepared = &owned.prepared;
        let child = &mut owned.child;
        if cancelled_exit(
            || prepared.cancel_before_exit(),
            || child.try_wait().map(|status| status.is_some()),
        )? {
            // Only an exited helper and a revalidated cancellation permit the
            // original GUI's shared access to be restored. Failure keeps the
            // project/installer leases here, and the parent remains frozen.
            parent_access.restore_parent_shared_access(prepared)?;
            self.owned.take();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Drop for WindowsParentHelper {
    fn drop(&mut self) {
        if let Some(mut owned) = self.owned.take() {
            // An accidental owner drop must not silently release pinned assets
            // or the project lease while an actual/unknown helper still runs.
            // Retain until parent process exit in this misuse/inspection case;
            // normal cancellation above releases promptly. Never force-kill.
            if retain_resources(owned.child.try_wait().map(|status| status.is_some())) {
                std::mem::forget(owned);
            }
        }
    }
}

fn readiness_result(
    elapsed: Duration,
    proof: io::Result<()>,
) -> io::Result<WindowsParentReadiness> {
    // A delayed acknowledgement cannot grant an unbounded close permission.
    if elapsed >= READY_TIMEOUT {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "helper readiness timed out; cancel or inspect",
        ));
    }
    match proof {
        Ok(()) => Ok(WindowsParentReadiness::Ready),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(WindowsParentReadiness::Waiting)
        }
        Err(error) => Err(error),
    }
}

fn recent_ready_gate(
    cancelling: bool,
    proof_age: Option<Duration>,
    observe_owned_exit: impl FnOnce() -> io::Result<bool>,
) -> io::Result<()> {
    if cancelling || proof_age.is_none_or(|age| age > Duration::from_millis(500)) {
        return Err(invalid("helper readiness proof is stale"));
    }
    if observe_owned_exit()? {
        return Err(invalid("owned helper exited before GUI close"));
    }
    Ok(())
}

fn cancelled_exit(
    mut commit_or_verify: impl FnMut() -> io::Result<()>,
    observe_owned_exit: impl FnOnce() -> io::Result<bool>,
) -> io::Result<bool> {
    commit_or_verify()?;
    if !observe_owned_exit()? {
        return Ok(false);
    }
    // Revalidate the immutable signal after actual exit. The caller must still
    // finalize/verify journal cancellation via restore_parent_shared_access;
    // this signal/exit check alone never authorizes unfreezing work.
    commit_or_verify()?;
    Ok(true)
}

fn retain_resources(exit: io::Result<bool>) -> bool {
    !matches!(exit, Ok(true))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn final_close_requires_fresh_proof_and_live_owned_child() {
        for age in [Duration::ZERO, Duration::from_millis(500)] {
            assert!(recent_ready_gate(false, Some(age), || Ok(false)).is_ok());
            assert!(recent_ready_gate(false, Some(age), || Ok(true)).is_err());
            assert!(
                recent_ready_gate(false, Some(age), || Err(io::ErrorKind::Other.into())).is_err()
            );
        }
        for age in [None, Some(Duration::from_millis(501))] {
            assert!(recent_ready_gate(false, age, || panic!("stale proof")).is_err());
        }
        assert!(recent_ready_gate(true, Some(Duration::ZERO), || panic!("cancelled")).is_err());
    }

    #[test]
    fn final_close_observes_real_exited_child() {
        let mut child = std::process::Command::new("cmd.exe")
            .args(["/D", "/C", "exit", "0"])
            .spawn()
            .unwrap();
        assert!(child.wait().unwrap().success());
        assert!(
            recent_ready_gate(false, Some(Duration::ZERO), || {
                child.try_wait().map(|status| status.is_some())
            })
            .is_err()
        );
    }

    #[test]
    fn readiness_is_bounded_and_only_exact_success_grants_ready() {
        assert_eq!(
            readiness_result(Duration::ZERO, Ok(())).unwrap(),
            WindowsParentReadiness::Ready
        );
        assert_eq!(
            readiness_result(Duration::ZERO, Err(io::ErrorKind::NotFound.into())).unwrap(),
            WindowsParentReadiness::Waiting
        );
        for kind in [
            io::ErrorKind::InvalidData,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Other,
        ] {
            assert_eq!(
                readiness_result(Duration::ZERO, Err(kind.into()))
                    .unwrap_err()
                    .kind(),
                kind
            );
        }
        for proof in [Ok(()), Err(io::ErrorKind::NotFound.into())] {
            assert_eq!(
                readiness_result(READY_TIMEOUT, proof).unwrap_err().kind(),
                io::ErrorKind::TimedOut
            );
        }
    }

    #[test]
    fn cancellation_requires_commit_then_actual_exit_then_final_verification() {
        assert!(
            cancelled_exit(
                || Err(invalid("commit failed")),
                || panic!("must not inspect exit before commit")
            )
            .is_err()
        );
        let calls = Cell::new(0);
        let commit = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        assert!(!cancelled_exit(commit, || Ok(false)).unwrap());
        assert_eq!(calls.get(), 1);
        calls.set(0);
        assert!(
            cancelled_exit(commit, || {
                assert_eq!(calls.get(), 1);
                Ok(true)
            })
            .unwrap()
        );
        assert_eq!(calls.get(), 2);
        calls.set(0);
        assert!(cancelled_exit(commit, || Err(io::ErrorKind::Other.into())).is_err());
        assert_eq!(calls.get(), 1);
        calls.set(0);
        assert!(
            cancelled_exit(
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 2 {
                        Err(invalid("state changed"))
                    } else {
                        Ok(())
                    }
                },
                || Ok(true)
            )
            .is_err()
        );
    }

    #[test]
    fn live_or_unknown_owned_child_never_releases_resources_on_drop() {
        assert!(retain_resources(Ok(false)));
        assert!(retain_resources(Err(io::ErrorKind::Other.into())));
        assert!(!retain_resources(Ok(true)));
    }
}
