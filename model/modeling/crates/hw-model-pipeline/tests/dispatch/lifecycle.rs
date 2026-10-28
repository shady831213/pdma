use super::fixtures::{Behavior, Context, Destination, Routing, Source, finish, request};
use hw_model_pipeline::{Dispatch, FanOut, Operation, StepOutcome};
#[test]
fn routes_owned_input_only_to_selected_destination_across_distinct_steps() {
    // Given: a request addressed to the middle destination.
    let mut hardware = FanOut::new(
        Source,
        Dispatch::new(Routing::Accept),
        (0..3).map(Destination::new).collect(),
    );
    let mut operation = Operation::new(request(1));
    let mut context = Context::default();
    // When: input collection and routing run before output execution.
    for _ in 0..2 {
        assert_eq!(
            operation.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    assert!(context.deliveries.is_empty());
    let result = operation.step(&mut hardware, &mut context);
    // Then: ownership reaches only the addressed output.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(1, ())]))));
    assert_eq!(context.deliveries, [(1, "owned payload".into())]);
}
#[test]
fn stalled_destination_resumes_owned_input_without_restarting_source() {
    // Given: the selected destination stalls after accepting its input.
    let mut hardware = FanOut::new(
        Source,
        Dispatch::new(Routing::Accept),
        vec![Destination {
            id: 0,
            behavior: Behavior::Stall,
            pending: (None, false),
        }],
    );
    let mut state = hw_model_pipeline::Operation::new(request(0));
    let mut context = Context::default();
    for _ in 0..6 {
        assert_eq!(
            state.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    // When: the destination becomes runnable.
    context.released = true;
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: the source ran once and the owned payload was retained.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(0, ())]))));
    assert_eq!(context.source_calls, 1);
    assert_eq!(context.deliveries, [(0, "owned payload".into())]);
}
#[test]
fn progressed_destination_retains_mutated_child_state() {
    // Given: output hardware modifies the payload on its first step.
    let mut hardware = FanOut::new(
        Source,
        Dispatch::new(Routing::Accept),
        vec![Destination {
            id: 0,
            behavior: Behavior::Progress,
            pending: (None, false),
        }],
    );
    let mut state = hw_model_pipeline::Operation::new(request(0));
    let mut context = Context::default();
    // When: execution proceeds to completion.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: the second step sees the original state mutation.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(0, ())]))));
    assert_eq!(context.deliveries, [(0, "owned payload advanced".into())]);
}
