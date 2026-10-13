//! External hardware driving boundaries.

use hardware_interface_pipeline::{HardwareDriver, RunError, Stage, StepOutcome, StepResult};

#[derive(Clone, Copy)]
enum Finish {
    Success,
    Failure,
    CallError,
}

struct Service(Finish);

impl Stage<Vec<&'static str>> for Service {
    type Input = ();
    type Output = ();
    type Error = &'static str;
    type State = bool;

    fn start((): ()) -> bool {
        false
    }

    fn step(
        &mut self,
        log: &mut Vec<&'static str>,
        accepted: &mut bool,
    ) -> StepResult<(), Self::Error> {
        if !*accepted {
            *accepted = true;
            log.push("accept");
            return Ok(StepOutcome::InProgress);
        }
        log.push("finish");
        match self.0 {
            Finish::Success => Ok(StepOutcome::Complete(Ok(()))),
            Finish::Failure => Ok(StepOutcome::Complete(Err("failed"))),
            Finish::CallError => Err(RunError::BudgetExhausted),
        }
    }
}

#[test]
fn successful_service_starts_fresh_only_on_the_next_external_step() {
    // Given: a service accepts and finishes through separate bounded steps.
    let mut hardware = Service(Finish::Success);
    let mut driver = HardwareDriver::new();
    let mut log = Vec::new();

    // When: external calls cross two finite service boundaries.
    for expected in [
        vec!["accept"],
        vec!["accept", "finish"],
        vec!["accept", "finish", "accept"],
        vec!["accept", "finish", "accept", "finish"],
    ] {
        let result = driver.step(&mut hardware, &mut log);

        // Then: each call performs one effect and successful driving stays live.
        assert_eq!(result, Ok(StepOutcome::InProgress));
        assert_eq!(log, expected);
        assert!(!driver.is_complete());
    }
}

#[test]
fn failures_stop_hardware_without_starting_another_service() {
    // Given: typed stage failure and raw call failure at a finite boundary.
    for (finish, expected) in [
        (Finish::Failure, Ok(StepOutcome::Complete(Err("failed")))),
        (Finish::CallError, Err(RunError::BudgetExhausted)),
    ] {
        let mut hardware = Service(finish);
        let mut driver = HardwareDriver::default();
        let mut log = Vec::new();
        assert_eq!(
            driver.step(&mut hardware, &mut log),
            Ok(StepOutcome::InProgress)
        );

        // When: failure is delivered and callers try driving again.
        let failed = driver.step(&mut hardware, &mut log);
        let retried = driver.step(&mut hardware, &mut log);

        // Then: failure is delivered once, and terminal hardware has no further effects.
        assert_eq!(failed, expected);
        assert_eq!(retried, Err(RunError::Exhausted));
        assert!(driver.is_complete());
        assert_eq!(log, ["accept", "finish"]);
    }
}
