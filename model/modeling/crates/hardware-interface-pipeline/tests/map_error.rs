//! Application error adaptation contracts.

use hardware_interface_pipeline::{MapError, Operation, Stage, StepOutcome, StepResult};

struct Fails;

impl Stage<()> for Fails {
    type Input = ();
    type Output = ();
    type Error = u8;
    type State = ();
    fn start((): ()) {}
    fn step(&mut self, (): &mut (), (): &mut ()) -> StepResult<(), u8> {
        Ok(StepOutcome::Complete(Err(7)))
    }
}

#[test]
fn application_error_adapter_maps_failure() {
    // Given: a ready source whose error must match a downstream pipeline.
    let mut stage = MapError::new(Fails, |code| format!("fault {code}"));
    // When: the operation is observed through the adapter.
    let result = Operation::new(()).step(&mut stage, &mut ());
    // Then: the adapter changes the error representation.
    assert_eq!(result, Ok(StepOutcome::Complete(Err("fault 7".to_owned()))));
}
