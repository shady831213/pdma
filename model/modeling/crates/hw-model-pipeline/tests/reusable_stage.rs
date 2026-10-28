//! Reusable hardware and separately retained request lifecycle checks.

use hw_model_pipeline::{Operation, PipelineExt, RunError, Stage, StepOutcome, StepResult};

#[derive(Default)]
struct Accumulator {
    calls: usize,
}

impl Stage<()> for Accumulator {
    type Input = String;
    type Output = String;
    type Error = String;

    fn step(&mut self, (): &mut (), state: &mut Option<Self::Input>) -> StepResult<String, String> {
        match state.take() {
            Some(input) => {
                self.calls += 1;
                Ok(StepOutcome::Complete(Ok(format!("{input}/{}", self.calls))))
            }
            None => Err(RunError::Exhausted),
        }
    }
}

#[test]
fn same_nested_hardware_processes_two_owned_requests() {
    // Given: three persistent hardware stages, with a composite in the downstream position.
    let mut hardware =
        Accumulator::default().then(Accumulator::default().then(Accumulator::default()));

    // When: independent operations pass through exactly the same hardware objects.
    for (input, expected) in [("first", "first/1/1/1"), ("second", "second/2/2/2")] {
        let mut operation = Operation::new(input.to_owned());
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            Ok(StepOutcome::InProgress)
        );
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            Ok(StepOutcome::InProgress)
        );

        // Then: each stage retains its hardware counter; only the completed operation exhausts.
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            Ok(StepOutcome::Complete(Ok(expected.to_owned())))
        );
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            Err(RunError::Exhausted)
        );
    }
}

#[derive(Default)]
struct Publication {
    ready: bool,
    records: Vec<String>,
    reset: bool,
}

impl Stage<()> for Publication {
    type Input = String;
    type Output = ();
    type Error = String;

    fn step(&mut self, (): &mut (), state: &mut Option<Self::Input>) -> StepResult<(), String> {
        if self.reset {
            state.take();
            return Ok(StepOutcome::Complete(Err("reset".to_owned())));
        }
        if !self.ready {
            return Ok(StepOutcome::InProgress);
        }
        match state.take() {
            Some(record) => {
                self.records.push(record);
                Ok(StepOutcome::Complete(Ok(())))
            }
            None => Err(RunError::Exhausted),
        }
    }
}

#[test]
fn stage_observes_reset_before_fresh_handoff_without_resetting_hardware() {
    // Given: a request retained downstream after an upstream hardware interaction.
    let mut hardware = Accumulator::default().then(Publication::default());
    let mut operation = Operation::new("discarded".to_owned());
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        Ok(StepOutcome::InProgress)
    );

    // When: the stage observes reset and completes before accepting a fresh request.
    hardware.downstream.reset = true;
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        Ok(StepOutcome::Complete(Err("reset".to_owned())))
    );
    hardware.downstream.reset = false;
    hardware.downstream.ready = true;
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        Err(RunError::Exhausted)
    );
    let mut fresh = Operation::new("fresh".to_owned());
    assert_eq!(
        fresh.step(&mut hardware, &mut ()),
        Ok(StepOutcome::InProgress)
    );
    assert_eq!(
        fresh.step(&mut hardware, &mut ()),
        Ok(StepOutcome::Complete(Ok(())))
    );

    // Then: upstream state persists and only the fresh request publishes.
    assert_eq!(hardware.upstream.calls, 2);
    assert_eq!(hardware.downstream.records, ["fresh/2"]);
}
