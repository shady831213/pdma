use super::fixtures::{
    Context, Destination, Parcel, Receipt, Request, Routing, Source, finish, request,
};
use hardware_interface_pipeline::{FanOut, StepOutcome};

#[test]
fn dispatches_only_selected_destination_across_distinct_steps() {
    // Given: an owned request for one destination.
    let mut hardware = FanOut::new(Source, Routing, (0..3).map(Destination::new).collect());
    let mut state = hardware_interface_pipeline::Operation::new(request(&[1]));
    let mut context = Context::default();
    // When: input and planning each run separately.
    for _ in 0..2 {
        assert_eq!(
            state.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    assert!(context.deliveries.is_empty());
    let result = state.step(&mut hardware, &mut context);
    // Then: only the selected output receives the owned value.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(1, Receipt(1))]))));
    assert_eq!(context.deliveries, [(1, "payload 1".into())]);
}
#[test]
fn reversed_subset_routes_payloads_to_their_selected_ports() {
    // Given: a reversed selection of distinct output ports.
    let mut hardware = FanOut::new(Source, Routing, (0..4).map(Destination::new).collect());
    let mut state = hardware_interface_pipeline::Operation::new(request(&[3, 1]));
    let mut context = Context::default();
    // When: the batch completes.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: the output sweep is deterministic by port index and routing remains correct.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Ok(vec![
            (1, Receipt(1)),
            (3, Receipt(3))
        ])))
    );
    assert_eq!(
        context.deliveries,
        [(1, "payload 1".into()), (3, "payload 3".into())]
    );
}
#[test]
fn splits_non_clone_owned_payloads_without_framework_cloning() {
    // Given: two separately owned fragments of one String.
    let mut text = "abcdefg".to_owned();
    let tail = text.split_off(3);
    let input = Request(vec![(1, Parcel(text)), (0, Parcel(tail))]);
    let mut hardware = FanOut::new(
        Source,
        Routing,
        vec![Destination::new(0), Destination::new(1)],
    );
    let mut state = hardware_interface_pipeline::Operation::new(input);
    let mut context = Context::default();
    // When: owned fragments move through the fan.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: neither fragment is duplicated or lost.
    assert!(matches!(result, Ok(StepOutcome::Complete(Ok(_)))));
    assert_eq!(context.deliveries, [(0, "defg".into()), (1, "abc".into())]);
}
#[test]
fn broadcasts_using_application_defined_cloning() {
    // Given: the application explicitly clones its payload.
    let text = "broadcast".to_owned();
    let input = Request((0..3).map(|i| (i, Parcel(text.clone()))).collect());
    let mut hardware = FanOut::new(Source, Routing, (0..3).map(Destination::new).collect());
    let mut state = hardware_interface_pipeline::Operation::new(input);
    let mut context = Context::default();
    // When: all outputs receive their owned requests.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: each output commits once.
    assert!(matches!(result, Ok(StepOutcome::Complete(Ok(_)))));
    assert_eq!(
        context.deliveries,
        [(0, text.clone()), (1, text.clone()), (2, text)]
    );
}
#[test]
fn empty_plan_consumes_input_without_touching_destinations() {
    // Given: a request explicitly selecting no output.
    let mut hardware = FanOut::new(Source, Routing, vec![Destination::new(0)]);
    let mut state = hardware_interface_pipeline::Operation::new(request(&[]));
    let mut context = Context::default();
    // When: the finite batch drains.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: completion is empty and no destination ran.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![]))));
    assert!(context.attempts.is_empty());
}
