//! Independent-helper lifecycle, still without a production CLI/UI caller.
//! Parent-side work protection and compatibility gates must precede entry.
use super::{WindowsControllerPhase, WindowsHelperSession, WindowsUpdateController};
use std::io;
use std::time::Duration;

/// Run only inside the authenticated copied helper after external preflight.
/// Never return/drop owned children on inspection while any may still run.
/// No timeout authorizes killing them, discarding locks or retrying installation.
pub fn run_helper_after_preflight(
    helper: &WindowsHelperSession,
) -> io::Result<WindowsControllerPhase> {
    helper.record_runtime_phase("runtime-waiting-parent-exit");
    let mut controller = WindowsUpdateController::wait_after_preflight(helper)?;
    drive(&mut controller, || {
        std::thread::sleep(Duration::from_millis(250));
    })
}

trait HelperDriver {
    fn step(&mut self) -> io::Result<WindowsControllerPhase>;
    fn inspection_children_exited(&mut self) -> io::Result<bool>;
}

impl HelperDriver for WindowsUpdateController<'_> {
    fn step(&mut self) -> io::Result<WindowsControllerPhase> {
        self.poll()
    }

    fn inspection_children_exited(&mut self) -> io::Result<bool> {
        WindowsUpdateController::inspection_children_exited(self)
    }
}

fn drive(
    driver: &mut impl HelperDriver,
    mut wait: impl FnMut(),
) -> io::Result<WindowsControllerPhase> {
    let failure = loop {
        match driver.step() {
            Ok(
                phase @ (WindowsControllerPhase::Completed
                | WindowsControllerPhase::RolledBack
                | WindowsControllerPhase::FailedBeforeApply),
            ) => return Ok(phase),
            Ok(WindowsControllerPhase::InspectionRequired) => {
                break io::Error::other("helper inspection required; no work replayed");
            }
            Ok(_) => wait(),
            Err(error) => break error,
        }
    };
    // Do not call step again after failure: retain witnesses, observe only
    // owned native handles, and preserve unfinished durable evidence for review.
    loop {
        if matches!(driver.inspection_children_exited(), Ok(true)) {
            return Err(failure);
        }
        wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct Fixture {
        steps: VecDeque<io::Result<WindowsControllerPhase>>,
        exits: VecDeque<io::Result<bool>>,
    }

    impl HelperDriver for Fixture {
        fn step(&mut self) -> io::Result<WindowsControllerPhase> {
            self.steps.pop_front().expect("no replay after failure")
        }

        fn inspection_children_exited(&mut self) -> io::Result<bool> {
            self.exits.pop_front().expect("no premature helper release")
        }
    }

    #[test]
    fn only_exact_terminal_phases_finish_without_inspection() {
        for terminal in [
            WindowsControllerPhase::Completed,
            WindowsControllerPhase::RolledBack,
            WindowsControllerPhase::FailedBeforeApply,
        ] {
            let mut fixture = Fixture {
                steps: VecDeque::from([Ok(WindowsControllerPhase::Applying), Ok(terminal)]),
                exits: VecDeque::new(),
            };
            let mut waits = 0;
            assert_eq!(drive(&mut fixture, || waits += 1).unwrap(), terminal);
            assert_eq!(waits, 1);
        }
    }

    #[test]
    fn failure_retains_live_or_unknown_children_without_replaying() {
        for failure in [
            Err(io::Error::other("checkpoint failure")),
            Ok(WindowsControllerPhase::InspectionRequired),
        ] {
            let mut fixture = Fixture {
                steps: VecDeque::from([failure]),
                exits: VecDeque::from([
                    Ok(false),
                    Err(io::Error::other("native observation unavailable")),
                    Ok(false),
                    Ok(true),
                ]),
            };
            let mut waits = 0;
            assert!(drive(&mut fixture, || waits += 1).is_err());
            assert_eq!(waits, 3);
            assert!(fixture.steps.is_empty());
            assert!(fixture.exits.is_empty());
        }
    }
}
