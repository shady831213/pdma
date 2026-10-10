use hardware_interface_pipeline::{Operation, PipelineExt, RunOutcome, Stage, StepOutcome, Then};

use super::fixtures::{Effect, Fault, Prepare, Reply};

#[test]
fn preparation_upstream_pending_or_progressed_is_never_retried_in_a_step() {
    for (reply, expected) in [
        (
            Reply::Pending,
            Ok::<RunOutcome<u32, Fault, u32>, hardware_interface_pipeline::AlreadyComplete>(
                RunOutcome::Pending(10),
            ),
        ),
        (Reply::Progressed, Ok(RunOutcome::Progressed)),
    ] {
        // Given: preparation that cannot yet supply an effect request.
        let mut pipeline = Prepare {
            reply,
            ..Prepare::default()
        }
        .then(Effect::default());
        let state = Then::<Prepare, Effect>::start(10);
        let mut context = Vec::new();

        // When: the composed stage advances once.
        let outcome = pipeline.step(&mut context, state);

        // Then: preparation is called once and no effect interaction occurs.
        match outcome {
            StepOutcome::Pending(_, wait) => assert_eq!(Ok(RunOutcome::Pending(wait)), expected),
            StepOutcome::Progressed(_) => assert_eq!(Ok(RunOutcome::Progressed), expected),
            StepOutcome::Complete(_) => panic!("preparation must remain active"),
        }
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 0);
        assert!(context.is_empty());
    }
}

#[test]
fn newly_reached_effect_pending_or_progressed_is_never_retried_in_a_step() {
    for (reply, expected) in [
        (
            Reply::Pending,
            Ok::<RunOutcome<u32, Fault, u32>, hardware_interface_pipeline::AlreadyComplete>(
                RunOutcome::Pending(11),
            ),
        ),
        (Reply::Progressed, Ok(RunOutcome::Progressed)),
    ] {
        // Given: preparation before an effect stage with unfinished work.
        let mut pipeline = Prepare::default().then(Effect {
            reply,
            ..Effect::default()
        });
        let state = Then::<Prepare, Effect>::start(10);
        let mut context = Vec::new();
        let StepOutcome::Progressed(state) = pipeline.step(&mut context, state) else {
            panic!("preparation must hand off owned state");
        };

        // When: the composed stage advances once.
        let outcome = pipeline.step(&mut context, state);

        // Then: the effect result is forwarded after exactly one child call.
        match outcome {
            StepOutcome::Pending(_, wait) => assert_eq!(Ok(RunOutcome::Pending(wait)), expected),
            StepOutcome::Progressed(_) => assert_eq!(Ok(RunOutcome::Progressed), expected),
            StepOutcome::Complete(_) => panic!("effect must remain active"),
        }
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 1);
        assert_eq!(context, [11]);
    }
}

#[test]
fn newly_reached_preparation_pending_or_progressed_is_never_retried_in_a_step() {
    for (reply, expected) in [
        (
            Reply::Pending,
            Ok::<RunOutcome<u32, Fault, u32>, hardware_interface_pipeline::AlreadyComplete>(
                RunOutcome::Pending(11),
            ),
        ),
        (Reply::Progressed, Ok(RunOutcome::Progressed)),
    ] {
        // Given: an effect stage followed by preparation that needs another step.
        let mut pipeline = Effect::default().then(Prepare {
            reply,
            ..Prepare::default()
        });
        let state = Then::<Effect, Prepare>::start(10);
        let mut context = Vec::new();
        let StepOutcome::Progressed(state) = pipeline.step(&mut context, state) else {
            panic!("effect must hand off owned state");
        };

        // When: the composed stage advances once.
        let outcome = pipeline.step(&mut context, state);

        // Then: the tail runs once and cannot trigger a preparation retry loop.
        match outcome {
            StepOutcome::Pending(_, wait) => assert_eq!(Ok(RunOutcome::Pending(wait)), expected),
            StepOutcome::Progressed(_) => assert_eq!(Ok(RunOutcome::Progressed), expected),
            StepOutcome::Complete(_) => panic!("preparation must remain active"),
        }
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 1);
        assert_eq!(context, [10]);
    }
}

#[test]
fn preparation_failure_or_missing_work_prevents_later_effect_interaction() {
    for (reply, expected) in [
        (Reply::Failure, Ok(RunOutcome::Complete(Err(Fault::Failed)))),
        (
            Reply::Missing,
            Ok(RunOutcome::Complete(Err(Fault::Missing))),
        ),
    ] {
        // Given: two preparation stages followed by an effect, with preparation stopping.
        let mut pipeline = Prepare::default()
            .then(Prepare {
                reply,
                ..Prepare::default()
            })
            .then(Effect::default());
        let mut state = Operation::new(10);
        let mut context = Vec::new();
        assert_eq!(
            state.step(&mut pipeline, &mut context),
            Ok(RunOutcome::Progressed)
        );

        // When: preparation terminates after its first handoff.
        let outcome = state.step(&mut pipeline, &mut context);

        // Then: its terminal result propagates and the effect stage is untouched.
        assert_eq!(outcome, expected);
        assert_eq!(pipeline.downstream.calls, 0);
        assert!(context.is_empty());
    }
}

#[test]
fn operation_is_inert_after_preparation_handoff_terminal_outcomes() {
    for reply in [Reply::Complete, Reply::Failure, Reply::Missing] {
        // Given: an operation already completed by a child handoff.
        let mut pipeline = Prepare::default().then(Effect {
            reply,
            ..Effect::default()
        });
        let mut state = Operation::new(10);
        let mut context = Vec::new();
        assert_eq!(
            state.step(&mut pipeline, &mut context),
            Ok(RunOutcome::Progressed)
        );
        let terminal = state.step(&mut pipeline, &mut context);
        assert!(matches!(terminal, Ok(RunOutcome::Complete(_))));

        // When: callers drive the completed Operation again.
        let outcome = state.step(&mut pipeline, &mut context);

        // Then: no stage is called again and prior effects remain unchanged.
        assert_eq!(outcome, Err(hardware_interface_pipeline::AlreadyComplete));
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 1);
        assert_eq!(context, [11]);
    }
}
