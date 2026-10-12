//! Reusable hardware and separately retained request lifecycle checks.

use hardware_interface_pipeline::{Operation, PipelineExt, Stage, StepOutcome};

#[derive(Default)]
struct Accumulator {
    calls: usize,
}

impl Stage<()> for Accumulator {
    type Input = String;
    type Output = String;
    type Error = String;
    type Wait = ();
    type State = Option<String>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(&mut self, (): &mut (), state: &mut Self::State) -> StepOutcome<String, String, ()> {
        match state.take() {
            Some(input) => {
                self.calls += 1;
                StepOutcome::Complete(Ok(format!("{input}/{}", self.calls)))
            }
            None => StepOutcome::Exhausted,
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
            StepOutcome::Progressed
        );
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            StepOutcome::Progressed
        );

        // Then: each stage retains its hardware counter; only the completed operation exhausts.
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            StepOutcome::Complete(Ok(expected.to_owned()))
        );
        assert_eq!(
            operation.step(&mut hardware, &mut ()),
            StepOutcome::Exhausted
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
    type State = Option<String>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(&mut self, (): &mut (), state: &mut Self::State) -> StepOutcome<(), String, ()> {
        if !self.ready {
            return StepOutcome::Pending(());
        }
        match state.take() {
            Some(record) => {
                self.records.push(record);
                StepOutcome::Complete(Ok(()))
            }
            None => StepOutcome::Exhausted,
        }
    }
}

#[test]
fn reset_discards_pending_handoff_without_resetting_hardware() {
    // Given: a request retained downstream after an upstream hardware interaction.
    let mut hardware = Accumulator::default().then(Publication::default());
    let mut operation = Operation::new("discarded".to_owned());
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        StepOutcome::Progressed
    );
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        StepOutcome::Pending(())
    );

    // When: reset discards that operation and a fresh request uses the same hardware.
    operation.reset();
    hardware.downstream.ready = true;
    assert_eq!(
        operation.step(&mut hardware, &mut ()),
        StepOutcome::Exhausted
    );
    let mut fresh = Operation::new("fresh".to_owned());
    assert_eq!(fresh.step(&mut hardware, &mut ()), StepOutcome::Progressed);
    assert_eq!(
        fresh.step(&mut hardware, &mut ()),
        StepOutcome::Complete(Ok(()))
    );

    // Then: upstream state persists and only the fresh request publishes.
    assert_eq!(hardware.upstream.calls, 2);
    assert_eq!(hardware.downstream.records, ["fresh/2"]);
}
