use hardware_interface_pipeline::{PipelineExt, RunError, Stage, StepOutcome};

use super::fixtures::{Effect, Fault, Prepare, Reply};

#[test]
fn preparation_upstream_unfinished_is_never_retried_in_a_step() {
    let reply = Reply::InProgress;
    let expected = Ok(StepOutcome::InProgress);
    // Given: preparation that cannot yet supply an effect request.
    let mut pipeline = Prepare {
        reply,
        ..Prepare::default()
    }
    .then(Effect::default());
    let mut state = hardware_interface_pipeline::Operation::new(10);
    let mut context = Vec::new();

    // When: the composed stage advances once.
    let outcome = state.step(&mut pipeline, &mut context);

    // Then: preparation is called once and no effect interaction occurs.
    assert_eq!(outcome, expected);
    assert_eq!(pipeline.upstream.calls, 1);
    assert_eq!(pipeline.downstream.calls, 0);
    assert!(context.is_empty());
}

#[test]
fn newly_reached_effect_unfinished_is_never_retried_in_a_step() {
    let reply = Reply::InProgress;
    let expected = Ok(StepOutcome::InProgress);
    // Given: preparation before an effect stage with unfinished work.
    let mut pipeline = Prepare::default().then(Effect {
        reply,
        ..Effect::default()
    });
    let mut state = hardware_interface_pipeline::Operation::new(10);
    let mut context = Vec::new();
    assert_eq!(
        state.step(&mut pipeline, &mut context),
        Ok(StepOutcome::InProgress)
    );

    // When: the composed stage advances once.
    let outcome = state.step(&mut pipeline, &mut context);

    // Then: the effect result is forwarded after exactly one child call.
    assert_eq!(outcome, expected);
    assert_eq!(pipeline.upstream.calls, 1);
    assert_eq!(pipeline.downstream.calls, 1);
    assert_eq!(context, [11]);
}

#[test]
fn newly_reached_preparation_unfinished_is_never_retried_in_a_step() {
    let reply = Reply::InProgress;
    let expected = Ok(StepOutcome::InProgress);
    // Given: an effect stage followed by preparation that needs another step.
    let mut pipeline = Effect::default().then(Prepare {
        reply,
        ..Prepare::default()
    });
    let mut state = hardware_interface_pipeline::Operation::new(10);
    let mut context = Vec::new();
    assert_eq!(
        state.step(&mut pipeline, &mut context),
        Ok(StepOutcome::InProgress)
    );

    // When: the composed stage advances once.
    let outcome = state.step(&mut pipeline, &mut context);

    // Then: the tail runs once and cannot trigger a preparation retry loop.
    assert_eq!(outcome, expected);
    assert_eq!(pipeline.upstream.calls, 1);
    assert_eq!(pipeline.downstream.calls, 1);
    assert_eq!(context, [10]);
}

#[test]
fn preparation_failure_or_exhaustion_prevents_later_effect_interaction() {
    for (reply, expected) in [
        (Reply::Failure, Ok(StepOutcome::Complete(Err(Fault)))),
        (Reply::Exhausted, Err(RunError::Exhausted)),
    ] {
        // Given: two preparation stages followed by an effect, with preparation stopping.
        let mut pipeline = Prepare::default()
            .then(Prepare {
                reply,
                ..Prepare::default()
            })
            .then(Effect::default());
        let mut state = hardware_interface_pipeline::Operation::new(10);
        let mut context = Vec::new();
        assert_eq!(
            state.step(&mut pipeline, &mut context),
            Ok(StepOutcome::InProgress)
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
fn stage_completion_is_inert_while_raw_errors_retain_the_active_child() {
    for reply in [Reply::Complete, Reply::Failure, Reply::Exhausted] {
        // Given: a graph returning stage completion or a resumable call error.
        let mut pipeline = Prepare::default().then(Effect {
            reply,
            ..Effect::default()
        });
        let mut input = Some(10);
        let mut context = Vec::new();
        assert_eq!(
            pipeline.step(&mut context, &mut input),
            Ok(StepOutcome::InProgress)
        );
        let terminal = pipeline.step(&mut context, &mut input);
        assert!(matches!(
            terminal,
            Ok(StepOutcome::Complete(_)) | Err(RunError::Exhausted)
        ));

        // When: the graph is stepped again without a new input.
        let outcome = pipeline.step(&mut context, &mut input);

        // Then: completed children stay idle while the raw-error child resumes in place.
        assert_eq!(pipeline.upstream.calls, 1);
        match reply {
            Reply::Exhausted => {
                assert_eq!(outcome, Err(RunError::Exhausted));
                pipeline.downstream.reply = Reply::Complete;
                assert_eq!(
                    pipeline.step(&mut context, &mut input),
                    Ok(StepOutcome::Complete(Ok(12)))
                );
                assert_eq!(pipeline.downstream.calls, 3);
                assert_eq!(context, [11, 11, 11]);
            }
            Reply::Complete | Reply::Failure => {
                assert_eq!(outcome, Ok(StepOutcome::InProgress));
                assert_eq!(pipeline.downstream.calls, 1);
                assert_eq!(context, [11]);
            }
            Reply::InProgress => {
                assert_eq!(outcome, Ok(StepOutcome::InProgress));
                assert_eq!(pipeline.downstream.calls, 2);
                assert_eq!(context, [11, 11]);
            }
        }
    }
}
