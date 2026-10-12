use super::fixtures::{Behavior, Context, Destination, Routing, Source, finish, request};
use hardware_interface_pipeline::{BranchError, Dispatch, FanOut, RunError, Stage, StepOutcome};
type Routed = FanOut<Source, Dispatch<Routing>, Destination>;
#[test]
fn invalid_destination_fails_before_effects_including_empty_population() {
    // Given: target 2 does not exist in either tested population.
    for count in [0, 2] {
        let mut hardware = FanOut::new(
            Source,
            Dispatch::new(Routing::Accept),
            (0..count).map(Destination::new).collect(),
        );
        let mut state = Routed::start(request(2));
        let mut context = Context::default();
        // When: routing validates the target.
        let result = finish(&mut hardware, &mut state, &mut context);
        // Then: destination effects are absent and the index error is exact.
        assert_eq!(
            result,
            Ok(StepOutcome::Complete(Err(BranchError::InvalidIndex {
                index: 2,
                ports: count
            })))
        );
        assert!(context.attempts.is_empty());
    }
}
#[test]
fn policy_error_is_wrapped_once_before_destination_effects() {
    // Given: an application policy rejecting its completed input.
    let mut hardware = FanOut::new(
        Source,
        Dispatch::new(Routing::Reject),
        vec![Destination::new(0)],
    );
    let mut state = Routed::start(request(0));
    let mut context = Context::default();
    // When: routing rejects the request.
    let result = finish(&mut hardware, &mut state, &mut context);
    // Then: the policy error is propagated without invoking the destination.
    assert_eq!(
        result,
        Ok(StepOutcome::Complete(Err(BranchError::Application(
            "route failed"
        ))))
    );
    assert!(context.attempts.is_empty());
}
#[test]
fn destination_error_or_exhaustion_makes_direct_state_terminal() {
    // Given: either a business failure or call-contract violation in the output.
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
            Dispatch::new(Routing::Accept),
            vec![Destination { id: 0, behavior }],
        );
        let mut state = Routed::start(request(0));
        let mut context = Context::default();
        // When: the failed output is advanced and the caller attempts reuse.
        let result = finish(&mut hardware, &mut state, &mut context);
        let repeated = hardware.step(&mut context, &mut state);
        // Then: terminal state cannot repeat output effects.
        assert_eq!(result, expected);
        assert_eq!(repeated, Err(RunError::Exhausted));
        assert_eq!(context.attempts, [0]);
        assert!(state.is_none());
    }
}
