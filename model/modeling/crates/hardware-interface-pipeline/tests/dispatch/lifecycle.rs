use hardware_interface_pipeline::{Dispatch, Operation, RunOutcome};

use super::fixtures::{Behavior, Context, Destination, Routing, request};

#[test]
fn routes_owned_input_only_to_selected_destination_across_distinct_steps() {
    // Given: a non-Clone request, payload, and receipt with only a Route policy.
    let mut context = Context::default();
    let mut hardware = Dispatch::new((0..3).map(Destination::new).collect(), Routing::Accept);
    let mut state = Operation::new(request(1));
    assert_eq!(context.routes.get(), 0);

    // When: planning, destination execution, and reduction advance separately.
    let planned = state.step(&mut hardware, &mut context);
    let attempts_after_plan = context.attempts.clone();
    let delivered = state.step(&mut hardware, &mut context);
    let result = state.step(&mut hardware, &mut context);
    let terminal = state.step(&mut hardware, &mut context);
    let repeated = state.step(&mut hardware, &mut context);

    // Then: the transformed payload is delivered once and reduction returns unit.
    assert_eq!(planned, Ok(RunOutcome::Progressed));
    assert!(attempts_after_plan.is_empty());
    assert_eq!(delivered, Ok(RunOutcome::Progressed));
    assert_eq!(result, Ok(RunOutcome::Complete(Ok(()))));
    assert_eq!(terminal, Err(hardware_interface_pipeline::AlreadyComplete));
    assert_eq!(repeated, Err(hardware_interface_pipeline::AlreadyComplete));
    assert_eq!(context.deliveries, [(1, "routed owned payload".into())]);
    assert_eq!(context.attempts, [1]);
    assert_eq!(context.routes.get(), 1);
}

#[test]
fn pending_destination_resumes_owned_input_without_rerouting() {
    // Given: the selected destination waits while other destinations are available.
    let mut context = Context::default();
    let mut hardware = Dispatch::new(
        vec![
            Destination::new(0),
            Destination {
                id: 1,
                behavior: Behavior::Pending,
            },
        ],
        Routing::Accept,
    );
    let mut operation = Operation::new(request(1));

    // When: the child waits twice and is released before resuming.
    let planned = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    let pending_again = operation.step(&mut hardware, &mut context);
    let deliveries_before_release = context.deliveries.clone();
    context.released = true;
    let delivered = operation.step(&mut hardware, &mut context);
    let result = operation.step(&mut hardware, &mut context);

    // Then: waiting preserves the chosen destination and its original owned payload.
    assert_eq!(planned, Ok(RunOutcome::Progressed));
    assert_eq!(pending, Ok(RunOutcome::Pending("release")));
    assert_eq!(pending_again, Ok(RunOutcome::Pending("release")));
    assert!(deliveries_before_release.is_empty());
    assert_eq!(delivered, Ok(RunOutcome::Progressed));
    assert_eq!(result, Ok(RunOutcome::Complete(Ok(()))));
    assert_eq!(context.deliveries, [(1, "routed owned payload".into())]);
    assert_eq!(context.attempts, [1, 1, 1]);
    assert_eq!(context.routes.get(), 1);
}

#[test]
fn progressed_destination_retains_mutated_child_state() {
    // Given: the selected child mutates its parcel before reporting progress.
    let mut context = Context::default();
    let mut hardware = Dispatch::new(
        vec![Destination {
            id: 0,
            behavior: Behavior::Progress,
        }],
        Routing::Accept,
    );
    let mut operation = Operation::new(request(0));

    // When: planning, child progress, child completion, and reduction run.
    let outcomes: Vec<_> = (0..4)
        .map(|_| operation.step(&mut hardware, &mut context))
        .collect();

    // Then: progress does not restart the child or recompute its route.
    assert_eq!(
        outcomes,
        [
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Complete(Ok(())))
        ]
    );
    assert_eq!(
        context.deliveries,
        [(0, "routed owned payload advanced".into())]
    );
    assert_eq!(context.attempts, [0, 0]);
    assert_eq!(context.routes.get(), 1);
}
