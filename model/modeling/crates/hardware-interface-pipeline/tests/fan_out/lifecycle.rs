use hardware_interface_pipeline::{BranchError, FanOut, Operation, RunOutcome};

use super::{
    fixtures::{Behavior, Context, Destination, Parcel, Receipt, Report, Routing, dispatch},
    policies::Reject,
};

#[test]
fn pending_destination_retains_prior_receipt_and_blocks_later_deliveries() {
    // Given: destination 1 waits after destination 0 completes, with destination 2 queued.
    let mut context = Context::default();
    let mut stage = FanOut::new(
        vec![
            Destination::new(0),
            Destination {
                id: 1,
                behavior: Behavior::Pending,
            },
            Destination::new(2),
        ],
        Routing,
    );
    let mut operation = Operation::new(dispatch(&[0, 1, 2]));

    // When: the operation pauses twice, then resumes after the application releases it.
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let first = operation.step(&mut stage, &mut context);
    let pending = operation.step(&mut stage, &mut context);
    let pending_again = operation.step(&mut stage, &mut context);
    let before_release = context.deliveries.clone();
    context.released = true;
    let resumed = operation.step(&mut stage, &mut context);
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let complete = operation.step(&mut stage, &mut context);

    // Then: completed work and its receipt survive, and each delivery commits once.
    assert_eq!(first, Ok(RunOutcome::Progressed));
    assert_eq!(pending, Ok(RunOutcome::Pending("release")));
    assert_eq!(pending_again, Ok(RunOutcome::Pending("release")));
    assert_eq!(before_release, [(0, "payload 0".into())]);
    assert_eq!(resumed, Ok(RunOutcome::Progressed));
    assert_eq!(
        complete,
        Ok(RunOutcome::Complete(Ok(Report {
            label: "delivery".into(),
            receipts: vec![Receipt(0), Receipt(1), Receipt(2)],
        })))
    );
    assert_eq!(context.attempts, [0, 1, 1, 1, 2]);
    assert_eq!(context.deliveries.len(), 3);
    assert_eq!(context.plans.get(), 1);
}

#[test]
fn malformed_plans_fail_before_any_destination_interaction() {
    // Given: plans with a valid first destination followed by an invalid or duplicate index.
    let cases = [
        (vec![0, 2], BranchError::InvalidIndex { index: 2, ports: 2 }),
        (vec![0, 0], BranchError::DuplicateIndex { index: 0 }),
    ];

    // When: each complete plan is validated in its first frame.
    let outcomes: Vec<_> = cases
        .into_iter()
        .map(|(indices, expected)| {
            let mut context = Context::default();
            let mut hardware = FanOut::new(vec![Destination::new(0), Destination::new(1)], Routing);
            let mut state = Operation::new(dispatch(&indices));
            let result = state.step(&mut hardware, &mut context);
            let terminal = state.step(&mut hardware, &mut context);
            (result, terminal, context.attempts, expected)
        })
        .collect();

    // Then: even the valid first destination is untouched, and rejected state is terminal.
    for (result, terminal, attempts, expected) in outcomes {
        assert_eq!(result, Ok(RunOutcome::Complete(Err(expected))));
        assert_eq!(terminal, Err(hardware_interface_pipeline::AlreadyComplete));
        assert!(attempts.is_empty());
    }
}

#[test]
fn terminal_destination_stops_unscheduled_work_and_keeps_committed_effects() {
    // Given: a later destination fails or reports missing work after the first delivery commits.
    let cases = [
        (
            Behavior::Error,
            Ok(RunOutcome::Complete(Err(BranchError::Application(
                "destination failed",
            )))),
        ),
        (
            Behavior::Missing,
            Ok(RunOutcome::Complete(Err(BranchError::Application(
                "missing destination work",
            )))),
        ),
    ];

    // When: each operation encounters its terminal destination and is stepped again.
    let outcomes: Vec<_> = cases
        .into_iter()
        .map(|(behavior, expected)| {
            let mut context = Context::default();
            let mut hardware = FanOut::new(
                vec![
                    Destination::new(0),
                    Destination { id: 1, behavior },
                    Destination::new(2),
                ],
                Routing,
            );
            let mut state = Operation::new(dispatch(&[0, 1, 2]));
            assert_eq!(
                state.step(&mut hardware, &mut context),
                Ok(RunOutcome::Progressed)
            );
            let first = state.step(&mut hardware, &mut context);
            let terminal = state.step(&mut hardware, &mut context);
            let again = state.step(&mut hardware, &mut context);
            (first, terminal, again, context, expected)
        })
        .collect();

    // Then: committed delivery remains, destination 2 is untouched, and replay is inert.
    for (first, terminal, again, context, expected) in outcomes {
        assert_eq!(first, Ok(RunOutcome::Progressed));
        assert_eq!(terminal, expected);
        assert_eq!(again, Err(hardware_interface_pipeline::AlreadyComplete));
        assert_eq!(context.deliveries, [(0, "payload 0".into())]);
        assert_eq!(context.attempts, [0, 1]);
    }
}

#[test]
fn policy_errors_preserve_only_effects_already_committed() {
    // Given: application rejection either during planning or after receipt collection.
    let cases = [
        (Reject::Split, "split failed", 0),
        (Reject::Combine, "combine failed", 1),
    ];

    // When: each application policy rejects its operation.
    let outcomes: Vec<_> = cases
        .into_iter()
        .map(|(policy, expected, committed)| {
            let mut context = Context::default();
            let mut stage = FanOut::new(vec![Destination::new(0)], policy);
            let mut operation = Operation::new(Parcel("request".into()));
            if committed == 1 {
                assert_eq!(
                    operation.step(&mut stage, &mut context),
                    Ok(RunOutcome::Progressed)
                );
                assert_eq!(
                    operation.step(&mut stage, &mut context),
                    Ok(RunOutcome::Progressed)
                );
            }
            (
                operation.step(&mut stage, &mut context),
                context,
                expected,
                committed,
            )
        })
        .collect();

    // Then: the exact policy error propagates without rolling back completed delivery.
    for (result, context, expected, committed) in outcomes {
        assert_eq!(
            result,
            Ok(RunOutcome::Complete(Err(BranchError::Application(
                expected
            ))))
        );
        assert_eq!(context.deliveries.len(), committed);
    }
}

#[test]
fn reset_abandons_queued_delivery_without_reverting_committed_delivery() {
    // Given: a two-destination fan-out with retained owned payloads.
    let mut context = Context::default();
    let mut stage = FanOut::new(vec![Destination::new(0), Destination::new(1)], Routing);
    let mut operation = Operation::new(dispatch(&[0, 1]));

    // When: reset follows the first completed delivery.
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let first = operation.step(&mut stage, &mut context);
    operation.reset();
    let terminal = operation.step(&mut stage, &mut context);

    // Then: the remaining payload is abandoned and the first external effect survives.
    assert_eq!(first, Ok(RunOutcome::Progressed));
    assert_eq!(terminal, Err(hardware_interface_pipeline::AlreadyComplete));
    assert_eq!(context.deliveries, [(0, "payload 0".into())]);
    assert_eq!(context.attempts, [0]);
}
