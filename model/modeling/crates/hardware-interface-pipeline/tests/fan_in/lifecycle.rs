use super::{
    policy::Policy,
    sources::{Accept, Candidate, Context, finish},
};
use hardware_interface_pipeline::{BranchError, FanIn, Operation, RunError, StepOutcome};
#[test]
fn invalidated_candidate_is_rechecked_before_queue_acceptance() {
    // Given: a candidate value was collected before its queue changed.
    let mut hardware = FanIn::new(
        vec![Candidate(0)],
        Policy::Fixed,
        Accept::new(super::sources::Behavior::Normal),
    );
    let mut state = hardware_interface_pipeline::Operation::new(vec![()]);
    let mut context = Context::with_jobs(&[0]);
    assert_eq!(
        state.step(&mut hardware, &mut context),
        Ok(StepOutcome::InProgress)
    );
    context.queues.clear();
    // When: the selected output checks its input against current hardware.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: stale readiness causes no acceptance.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![(0, None)]))));
    assert!(context.accepted.is_empty());
}
#[test]
fn stalled_grant_retains_accepted_state_after_readiness_disappears() {
    // Given: accepting a queued job removes its candidate readiness.
    let mut hardware = FanIn::new(
        vec![Candidate(0)],
        Policy::Fixed,
        Accept::new(super::sources::Behavior::Normal),
    );
    let mut state = hardware_interface_pipeline::Operation::new(vec![()]);
    let mut context = Context::with_jobs(&[0]);
    context.blocked.push(0);
    for _ in 0..100 {
        assert_eq!(
            state.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
        if context.stalled {
            break;
        }
    }
    assert_eq!(context.accepted, [0]);
    assert!(context.stalled);
    // When: the accepted job is released.
    context.blocked.clear();
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: it completes without refetching or rescanning the finished input.
    assert!(matches!(result, Ok(StepOutcome::Complete(Ok(_)))));
    assert_eq!(context.accepted, [0]);
    assert_eq!(context.observed, [0]);
}
#[test]
fn terminal_outcomes_are_forwarded_and_direct_state_is_inert() {
    // Given: policy and child failures at distinct lifecycle boundaries.
    for (policy, accept, expected) in [
        (
            Policy::Reject,
            Accept::new(super::sources::Behavior::Normal),
            Ok(StepOutcome::Complete(Err(BranchError::Application(
                "selection failed",
            )))),
        ),
        (
            Policy::Invalid(3),
            Accept::new(super::sources::Behavior::Normal),
            Ok(StepOutcome::Complete(Err(BranchError::InvalidIndex {
                index: 3,
                ports: 1,
            }))),
        ),
        (
            Policy::Fixed,
            Accept::new(super::sources::Behavior::Failure),
            Ok(StepOutcome::Complete(Err(BranchError::Application(
                "accept failed",
            )))),
        ),
        (
            Policy::Fixed,
            Accept::new(super::sources::Behavior::Exhausted),
            Err(RunError::Exhausted),
        ),
    ] {
        let mut hardware = FanIn::new(vec![Candidate(0)], policy, accept);
        let mut state = hardware_interface_pipeline::Operation::new(vec![()]);
        let mut context = Context::with_jobs(&[0]);
        // When: each terminal outcome is encountered.
        let result = finish(&mut hardware, &mut state, &mut context);
        // Then: the original failure remains distinguishable and repeated stepping is inert.
        assert_eq!(result, expected);
        if expected.is_err() {
            assert!(!state.is_complete());
            assert!(matches!(
                finish(&mut hardware, &mut state, &mut context),
                Ok(StepOutcome::Complete(Ok(_)))
            ));
            assert_eq!(context.accepted, [0]);
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
fn empty_fan_completes_without_interaction() {
    // Given: a finite batch with no input stages.
    let mut hardware = FanIn::<Candidate, _, _, _, _, _, _, _>::new(
        vec![],
        Policy::Fixed,
        Accept::new(super::sources::Behavior::Normal),
    );
    let mut state = hardware_interface_pipeline::Operation::new(vec![]);
    let mut context = Context::default();
    // When: the batch drains.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: empty completion requires no source or destination interaction.
    assert_eq!(result, Ok(StepOutcome::Complete(Ok(vec![]))));
    assert!(context.observed.is_empty());
    assert!(context.accepted.is_empty());
}

#[test]
fn accepted_output_observes_reset_without_replaying_external_effects() {
    // Given: output hardware retains an accepted job until software releases it.
    let mut hardware = FanIn::new(
        vec![Candidate(0)],
        Policy::Fixed,
        Accept::new(super::sources::Behavior::Normal),
    );
    let mut operation = Operation::new(vec![()]);
    let mut context = Context::with_jobs(&[0]);
    context.blocked.push(0);
    for _ in 0..100 {
        assert_eq!(
            operation.step(&mut hardware, &mut context),
            Ok(StepOutcome::InProgress)
        );
        if context.stalled {
            break;
        }
    }
    assert_eq!(context.accepted, [0]);
    assert!(context.stalled);
    // When: the accepted output observes a domain reset despite its external stall.
    context.reset = true;
    assert_eq!(
        finish(&mut hardware, &mut operation, &mut context),
        Ok(StepOutcome::Complete(Err(BranchError::Application(
            "reset"
        ))))
    );
    context.blocked.clear();
    // Then: accepted external effects remain and no invocation work can restart.
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Err(RunError::Exhausted)
    );
    assert_eq!(context.accepted, [0]);
    assert_eq!(context.observed, [0]);
}
