use hardware_interface_pipeline::{PipelineExt, Stage, StepOutcome, Then};

use super::fixtures::{Effect, Fault, Prepare, Reply};

#[test]
fn preparation_upstream_pending_or_progressed_is_never_retried_in_a_step() {
    for (reply, expected) in [
        (Reply::Pending, StepOutcome::Pending(10)),
        (Reply::Progressed, StepOutcome::Progressed),
    ] {
        // Given: preparation that cannot yet supply an effect request.
        let mut pipeline = Prepare {
            reply,
            ..Prepare::default()
        }
        .then(Effect::default());
        let mut state = Then::<Prepare, Effect>::start(10);
        let mut context = Vec::new();

        // When: the composed stage advances once.
        let outcome = pipeline.step(&mut context, &mut state);

        // Then: preparation is called once and no effect interaction occurs.
        assert_eq!(outcome, expected);
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 0);
        assert!(context.is_empty());
    }
}

#[test]
fn newly_reached_effect_pending_or_progressed_is_never_retried_in_a_step() {
    for (reply, expected) in [
        (Reply::Pending, StepOutcome::Pending(11)),
        (Reply::Progressed, StepOutcome::Progressed),
    ] {
        // Given: preparation before an effect stage with unfinished work.
        let mut pipeline = Prepare::default().then(Effect {
            reply,
            ..Effect::default()
        });
        let mut state = Then::<Prepare, Effect>::start(10);
        let mut context = Vec::new();
        assert_eq!(
            pipeline.step(&mut context, &mut state),
            StepOutcome::Progressed
        );

        // When: the composed stage advances once.
        let outcome = pipeline.step(&mut context, &mut state);

        // Then: the effect result is forwarded after exactly one child call.
        assert_eq!(outcome, expected);
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 1);
        assert_eq!(context, [11]);
    }
}

#[test]
fn newly_reached_preparation_pending_or_progressed_is_never_retried_in_a_step() {
    for (reply, expected) in [
        (Reply::Pending, StepOutcome::Pending(11)),
        (Reply::Progressed, StepOutcome::Progressed),
    ] {
        // Given: an effect stage followed by preparation that needs another step.
        let mut pipeline = Effect::default().then(Prepare {
            reply,
            ..Prepare::default()
        });
        let mut state = Then::<Effect, Prepare>::start(10);
        let mut context = Vec::new();
        assert_eq!(
            pipeline.step(&mut context, &mut state),
            StepOutcome::Progressed
        );

        // When: the composed stage advances once.
        let outcome = pipeline.step(&mut context, &mut state);

        // Then: the tail runs once and cannot trigger a preparation retry loop.
        assert_eq!(outcome, expected);
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 1);
        assert_eq!(context, [10]);
    }
}

#[test]
fn preparation_failure_or_exhaustion_prevents_later_effect_interaction() {
    for (reply, expected) in [
        (Reply::Failure, StepOutcome::Complete(Err(Fault))),
        (Reply::Exhausted, StepOutcome::Exhausted),
    ] {
        // Given: two preparation stages followed by an effect, with preparation stopping.
        let mut pipeline = Prepare::default()
            .then(Prepare {
                reply,
                ..Prepare::default()
            })
            .then(Effect::default());
        let mut state = Then::<Then<Prepare, Prepare>, Effect>::start(10);
        let mut context = Vec::new();
        assert_eq!(
            pipeline.step(&mut context, &mut state),
            StepOutcome::Progressed
        );

        // When: preparation terminates after its first handoff.
        let outcome = pipeline.step(&mut context, &mut state);

        // Then: its terminal result propagates and the effect stage is untouched.
        assert_eq!(outcome, expected);
        assert_eq!(pipeline.downstream.calls, 0);
        assert!(context.is_empty());
    }
}

#[test]
fn direct_state_is_inert_after_preparation_handoff_terminal_outcomes() {
    for reply in [Reply::Complete, Reply::Failure, Reply::Exhausted] {
        // Given: direct state already terminalized by a child handoff.
        let mut pipeline = Prepare::default().then(Effect {
            reply,
            ..Effect::default()
        });
        let mut state = Then::<Prepare, Effect>::start(10);
        let mut context = Vec::new();
        assert_eq!(
            pipeline.step(&mut context, &mut state),
            StepOutcome::Progressed
        );
        let terminal = pipeline.step(&mut context, &mut state);
        assert!(matches!(
            terminal,
            StepOutcome::Complete(_) | StepOutcome::Exhausted
        ));

        // When: direct callers step terminal state again without an Operation wrapper.
        let outcome = pipeline.step(&mut context, &mut state);

        // Then: no stage is called again and prior effects remain unchanged.
        assert_eq!(outcome, StepOutcome::Exhausted);
        assert_eq!(pipeline.upstream.calls, 1);
        assert_eq!(pipeline.downstream.calls, 1);
        assert_eq!(context, [11]);
    }
}
