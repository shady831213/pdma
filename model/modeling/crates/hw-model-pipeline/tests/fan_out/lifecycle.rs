use super::fixtures::{Behavior, Context, Destination, Receipt, Routing, Source, finish, request};
use super::policies::Reject;
use hw_model_pipeline::{BranchError, FanOut, Operation, RunError, StepOutcome};
#[test]
fn stalled_destination_does_not_block_peers_or_lose_receipts() {
    // Given: the middle output is blocked.
    let mut hardware = FanOut::new(Source, Routing, (0..3).map(Destination::new).collect());
    let mut state = hw_model_pipeline::Operation::new(request(&[0, 1, 2]));
    let mut context = Context {
        blocked: vec![1],
        ..Context::default()
    };
    // When: peers finish and the blocked output later resumes.
    for _ in 0..6 {
        assert_eq!(
            state.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    assert_eq!(
        context.deliveries,
        [(0, "payload 0".into()), (2, "payload 2".into())]
    );
    context.blocked.clear();
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: retained receipts and payload are delivered once.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Ok(vec![
            (0, Receipt(0)),
            (2, Receipt(2)),
            (1, Receipt(1))
        ])))
    );
    assert_eq!(context.sources, 1);
}
#[test]
fn malformed_plans_fail_before_destination_interaction() {
    // Given: invalid and duplicate destinations follow a valid index.
    for (indices, expected) in [
        (vec![0, 2], BranchError::InvalidIndex { index: 2, ports: 2 }),
        (vec![0, 0], BranchError::DuplicateIndex { index: 0 }),
    ] {
        let mut hardware = FanOut::new(Source, Routing, (0..2).map(Destination::new).collect());
        let mut state = hw_model_pipeline::Operation::new(request(&indices));
        let mut context = Context::default();
        // When: the complete plan is validated.
        let result = finish(&mut hardware, &mut state, &mut context);
        // Then: validation prevents even the valid destination from running.
        assert_eq!(result, Ok(StepOutcome::Complete(Err(expected))));
        assert!(context.attempts.is_empty());
        assert_eq!(
            state.step(&mut hardware, &mut context),
            Err(RunError::Exhausted)
        );
    }
}
#[test]
fn terminal_destination_stops_later_work_and_keeps_committed_effects() {
    // Given: a later destination fails or violates its call contract.
    for (behavior, expected) in [
        (
            Behavior::Error,
            Ok(StepOutcome::Complete(Err(BranchError::Application(
                "destination failed",
            )))),
        ),
        (Behavior::Exhausted, Err(RunError::Exhausted)),
    ] {
        let mut hardware = FanOut::new(
            Source,
            Routing,
            vec![
                Destination::new(0),
                Destination {
                    id: 1,
                    behavior,
                    pending: (None, false),
                },
                Destination::new(2),
            ],
        );
        let mut state = hw_model_pipeline::Operation::new(request(&[0, 1, 2]));
        let mut context = Context::default();
        // When: the output sweep reaches the failed destination.
        let result = finish(&mut hardware, &mut state, &mut context);
        // Then: the first effect remains and later destinations never run.
        assert_eq!(result, expected);
        assert_eq!(context.deliveries, [(0, "payload 0".into())]);
        assert_eq!(context.attempts, [0, 1]);
        if expected.is_err() {
            assert!(!state.is_complete());
            assert_eq!(
                finish(&mut hardware, &mut state, &mut context),
                Ok(StepOutcome::Complete(Ok(vec![
                    (0, Receipt(0)),
                    (1, Receipt(1)),
                    (2, Receipt(2))
                ])))
            );
            assert_eq!(
                context.deliveries,
                [
                    (0, "payload 0".into()),
                    (1, "payload 1".into()),
                    (2, "payload 2".into())
                ]
            );
        } else {
            assert!(state.is_complete());
            assert_eq!(
                state.step(&mut hardware, &mut context),
                Err(RunError::Exhausted)
            );
        }
    }
}
#[test]
fn policy_errors_terminalize_without_destination_effects() {
    // Given: either policy phase rejects the request.
    for (policy, message) in [
        (Reject::Plan, "plan failed"),
        (Reject::Route, "route failed"),
    ] {
        let mut hardware = FanOut::new(Source, policy, vec![Destination::new(0)]);
        let mut state = hw_model_pipeline::Operation::new(request(&[0]));
        let mut context = Context::default();
        // When: policy evaluation fails.
        let result = finish(&mut hardware, &mut state, &mut context);
        // Then: the exact application error is returned once.
        assert_eq!(
            result,
            Ok(StepOutcome::Complete(Err(BranchError::Application(
                message
            ))))
        );
        assert!(context.attempts.is_empty());
        assert!(state.is_complete());
    }
}
#[test]
fn stalled_destination_observes_reset_without_reverting_committed_delivery() {
    // Given: one output can commit while its peer is blocked.
    let mut hardware = FanOut::new(Source, Routing, (0..2).map(Destination::new).collect());
    let mut operation = Operation::new(request(&[0, 1]));
    let mut context = Context {
        blocked: vec![1],
        ..Context::default()
    };
    for _ in 0..3 {
        assert_eq!(
            operation.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
    }
    // When: the stalled destination observes reset and finishes its own invocation.
    context.reset = true;
    assert_eq!(
        finish(&mut hardware, &mut operation, &mut context),
        Ok(StepOutcome::Complete(Err(BranchError::Application(
            "reset"
        ))))
    );
    // Then: committed effects remain and outstanding work is inert.
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Err(RunError::Exhausted)
    );
    assert_eq!(context.deliveries, [(0, "payload 0".into())]);
}
