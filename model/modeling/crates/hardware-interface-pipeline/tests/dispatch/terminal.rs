use hardware_interface_pipeline::{BranchError, Dispatch, Stage, StepOutcome};

use super::fixtures::{Behavior, Context, Destination, Routing, request};

#[test]
fn invalid_destination_fails_before_effects_including_empty_population() {
    // Given: a one-past-end selection and an empty destination population.
    let cases = [(2, 2), (0, 0)];

    // When: each route is planned and the failed state is stepped repeatedly.
    let results: Vec<_> = cases
        .into_iter()
        .map(|(ports, index)| {
            let mut context = Context::default();
            let mut hardware =
                Dispatch::new((0..ports).map(Destination::new).collect(), Routing::Accept);
            let mut state =
                <Dispatch<Destination, Routing> as Stage<Context>>::start(request(index));
            let result = hardware.step(&mut context, &mut state);
            let terminal = hardware.step(&mut context, &mut state);
            let repeated = hardware.step(&mut context, &mut state);
            (
                result,
                terminal,
                repeated,
                context,
                BranchError::InvalidIndex { index, ports },
            )
        })
        .collect();

    // Then: index validation precedes every destination interaction and replay is inert.
    for (result, terminal, repeated, context, expected) in results {
        assert_eq!(result, StepOutcome::Complete(Err(expected)));
        assert_eq!(terminal, StepOutcome::Exhausted);
        assert_eq!(repeated, StepOutcome::Exhausted);
        assert!(context.attempts.is_empty());
        assert!(context.deliveries.is_empty());
        assert_eq!(context.routes.get(), 1);
    }
}

#[test]
fn policy_error_is_wrapped_once_before_destination_effects() {
    // Given: an application Route policy that rejects the owned request.
    let mut context = Context::default();
    let mut hardware = Dispatch::new(vec![Destination::new(0)], Routing::Reject);
    let mut state = <Dispatch<Destination, Routing> as Stage<Context>>::start(request(0));

    // When: routing fails and the terminal state is stepped again.
    let result = hardware.step(&mut context, &mut state);
    let terminal = hardware.step(&mut context, &mut state);

    // Then: one Application wrapper carries the exact error with no destination attempt.
    assert_eq!(
        result,
        StepOutcome::Complete(Err(BranchError::Application("route failed")))
    );
    assert_eq!(terminal, StepOutcome::Exhausted);
    assert!(context.attempts.is_empty());
    assert!(context.deliveries.is_empty());
    assert_eq!(context.routes.get(), 1);
}

#[test]
fn destination_error_or_exhaustion_makes_direct_state_terminal() {
    // Given: a selected destination that fails or exhausts while another is available.
    let cases = [
        (
            Behavior::Error,
            StepOutcome::Complete(Err(BranchError::Application("destination failed"))),
        ),
        (Behavior::Exhausted, StepOutcome::Exhausted),
    ];

    // When: each child terminates and its state is stepped twice more.
    let results: Vec<_> = cases
        .into_iter()
        .map(|(behavior, expected)| {
            let mut context = Context::default();
            let mut hardware = Dispatch::new(
                vec![Destination { id: 0, behavior }, Destination::new(1)],
                Routing::Accept,
            );
            let mut state = <Dispatch<Destination, Routing> as Stage<Context>>::start(request(0));
            let planned = hardware.step(&mut context, &mut state);
            let result = hardware.step(&mut context, &mut state);
            let terminal = hardware.step(&mut context, &mut state);
            let repeated = hardware.step(&mut context, &mut state);
            (planned, result, terminal, repeated, context, expected)
        })
        .collect();

    // Then: errors stay singly wrapped and neither selected nor fallback work is replayed.
    for (planned, result, terminal, repeated, context, expected) in results {
        assert_eq!(planned, StepOutcome::Progressed);
        assert_eq!(result, expected);
        assert_eq!(terminal, StepOutcome::Exhausted);
        assert_eq!(repeated, StepOutcome::Exhausted);
        assert_eq!(context.attempts, [0]);
        assert!(context.deliveries.is_empty());
        assert_eq!(context.routes.get(), 1);
    }
}
