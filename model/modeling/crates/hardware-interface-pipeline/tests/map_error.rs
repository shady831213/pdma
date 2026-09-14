//! Application error adaptation contracts.

use hardware_interface_pipeline::{MapError, Operation, ReadyStage, Stage, StepOutcome};

struct Fails;

impl Stage<()> for Fails {
    type Input = ();
    type Output = ();
    type Error = u8;
    type Wait = ();
    type State = ();
    fn start((): ()) {}
    fn step(&mut self, (): &mut (), (): &mut ()) -> StepOutcome<(), u8, ()> {
        StepOutcome::Complete(Err(7))
    }
}

impl ReadyStage<()> for Fails {
    fn ready(&self, (): &()) -> bool {
        true
    }
}

#[test]
fn application_error_adapter_maps_failure_and_forwards_readiness() {
    // Given: a ready source whose error must match a downstream pipeline.
    let mut stage = MapError::new(Fails, |code| format!("fault {code}"));
    // When: readiness and the operation are observed through the adapter.
    let ready = stage.ready(&());
    let result = Operation::new(()).step(&mut stage, &mut ());
    // Then: readiness remains available and only the error representation changes.
    assert!(ready);
    assert_eq!(result, StepOutcome::Complete(Err("fault 7".to_owned())));
}
