use super::fixtures::{Behavior, Context, Destination, Receipt, Routing, Source, finish, request};
use hardware_interface_pipeline::{FanOut, RunError, StepOutcome};
#[test]
fn progressed_destination_retains_mutated_state_and_terminal_state_is_empty() {
    // Given: output hardware mutates owned state before completing.
    let mut hardware = FanOut::new(
        Source,
        Routing,
        vec![Destination {
            id: 0,
            behavior: Behavior::Progress,
            pending: (None, false),
        }],
    );
    let mut state = hardware_interface_pipeline::Operation::new(request(&[0]));
    let mut context = Context::default();
    // When: the invocation finishes through multiple output sweeps.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: local mutations survive and completion discards all local state.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(0, Receipt(0))]))));
    assert_eq!(context.deliveries, [(0, "payload 0!".into())]);
    assert!(state.is_complete());
    assert_eq!(
        state.step(&mut hardware, &mut context),
        Err(RunError::Exhausted)
    );
}
