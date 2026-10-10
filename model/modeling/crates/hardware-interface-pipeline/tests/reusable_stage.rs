//! Reusable hardware and separately retained request lifecycle checks.

use hardware_interface_pipeline::{Operation, PipelineExt, RunOutcome, Stage, StepOutcome};

#[derive(Default)]
struct Accumulator {
    calls: usize,
}

impl Stage<()> for Accumulator {
    type Input = String;
    type Output = String;
    type Error = String;
    type Wait = ();
    type State = String;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        (): &mut (),
        input: Self::State,
    ) -> StepOutcome<Self::State, String, String, ()> {
        self.calls += 1;
        StepOutcome::Complete(Ok(format!("{input}/{}", self.calls)))
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
            Ok(RunOutcome::Progressed)
        );
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            Ok(RunOutcome::Progressed)
        );

        // Then: each stage retains its hardware counter; only the completed operation exhausts.
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            Ok(RunOutcome::Complete(Ok(expected.to_owned())))
        );
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            Err(hardware_interface_pipeline::AlreadyComplete)
        );
    }
}

#[derive(Default)]
struct Publication {
    ready: bool,
    records: Vec<String>,
}

impl Stage<()> for Publication {
    type Input = String;
    type Output = ();
    type Error = String;
    type Wait = ();
    type State = String;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        (): &mut (),
        state: Self::State,
    ) -> StepOutcome<Self::State, (), String, ()> {
        if !self.ready {
            return StepOutcome::Pending(state, ());
        }
        self.records.push(state);
        StepOutcome::Complete(Ok(()))
    }
}

#[test]
fn reset_discards_pending_handoff_without_resetting_hardware() {
    // Given: a request retained downstream after an upstream hardware interaction.
    let mut hardware = Accumulator::default().then(Publication::default());
    let mut operation = Operation::new("discarded".to_owned());
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        Ok(RunOutcome::Progressed)
    );
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        Ok(RunOutcome::Pending(()))
    );

    // When: reset discards that operation and a fresh request uses the same hardware.
    operation.reset();
    hardware.downstream.ready = true;
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        Err(hardware_interface_pipeline::AlreadyComplete)
    );
    let mut fresh = Operation::new("fresh".to_owned());
    assert_eq!(
        fresh.step(&mut hardware, &mut ()),
        Ok(RunOutcome::Progressed)
    );
    assert_eq!(
        fresh.step(&mut hardware, &mut ()),
        Ok(RunOutcome::Complete(Ok(())))
    );

    // Then: upstream state persists and only the fresh request publishes.
    assert_eq!(hardware.upstream.calls, 2);
    assert_eq!(hardware.downstream.records, ["fresh/2"]);
}
