use super::fixtures::{Behavior, Context, Destination, Receipt, Routing, Source, finish, request};
use hardware_interface_pipeline::{FanOut, RunError, Stage, StepOutcome};
#[test]
fn progressed_destination_retains_mutated_state_and_terminal_state_is_empty() {
    // Given: output hardware mutates owned state before completing.
    let mut hardware = FanOut::new(
        Source,
        Routing,
        vec![Destination {
            id: 0,
            behavior: Behavior::Progress,
        }],
    );
    let mut state = <FanOut<Source, Routing, Destination> as Stage<Context>>::start(request(&[0]));
    let mut context = Context::default();
    // When: the invocation finishes through multiple output sweeps.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: local mutations survive and completion discards all local state.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(0, Receipt(0))]))));
    assert_eq!(context.deliveries, [(0, "payload 0!".into())]);
    assert!(state.is_none());
    assert_eq!(
        hardware.step(&mut context, &mut state),
        Err(RunError::Exhausted)
    );
}
