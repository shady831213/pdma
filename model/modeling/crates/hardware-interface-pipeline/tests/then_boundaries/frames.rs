use hardware_interface_pipeline::{Operation, PipelineExt, RunOutcome, StepExecutor, Then};

use super::fixtures::{Effect, Prepare};

#[test]
fn preparation_hands_off_without_executing_effect() {
    // Given: request preparation before an effect.
    let mut pipeline = Prepare::default().then(Effect::default());
    let mut operation = Operation::new(10);
    let mut context = Vec::new();

    // When: the executor advances one step.
    let outcome = StepExecutor.step(&mut pipeline, &mut operation, &mut context);

    // Then: only preparation runs.
    assert_eq!(outcome, Ok(RunOutcome::Progressed));
    assert_eq!(pipeline.upstream.calls, 1);
    assert_eq!(pipeline.downstream.calls, 0);
    assert!(context.is_empty());
}

#[test]
fn effect_hands_off_without_executing_preparation() {
    // Given: an effect before result preparation.
    let mut pipeline = Effect::default().then(Prepare::default());
    let mut operation = Operation::new(10);
    let mut context = Vec::new();

    // When: the executor advances one step.
    let outcome = StepExecutor.step(&mut pipeline, &mut operation, &mut context);

    // Then: the effect is committed and result preparation waits.
    assert_eq!(outcome, Ok(RunOutcome::Progressed));
    assert_eq!(context, [10]);
    assert_eq!(pipeline.downstream.calls, 0);
}

#[test]
fn left_nested_chain_runs_one_leaf_per_step() {
    // Given: preparation and reduction around two effects.
    let mut pipeline = Prepare::default()
        .then(Effect::default())
        .then(Prepare::default())
        .then(Effect::default());
    let mut operation = Operation::new(10);
    let mut context = Vec::new();

    // When: the executor drives the four leaves.
    let mut snapshots = Vec::new();
    let outcomes: Vec<_> = (0..4)
        .map(|_| {
            let outcome = StepExecutor.step(&mut pipeline, &mut operation, &mut context);
            snapshots.push(context.clone());
            outcome
        })
        .collect();

    // Then: handoffs preserve values and expose every leaf boundary.
    assert_eq!(
        outcomes,
        [
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Complete(Ok(14))),
        ]
    );
    assert_eq!(snapshots, [vec![], vec![11], vec![11], vec![11, 13]]);
}

#[test]
fn right_nested_chain_runs_one_leaf_per_step() {
    // Given: preparation before a right-nested effect/preparation/effect chain.
    let child = Effect::default().then(Prepare::default().then(Effect::default()));
    let mut pipeline = Prepare::default().then(child);
    let mut operation = Operation::new(10);
    let mut context = Vec::new();

    // When: the executor drives the four leaves.
    let mut snapshots = Vec::new();
    let outcomes: Vec<_> = (0..4)
        .map(|_| {
            let outcome = StepExecutor.step(&mut pipeline, &mut operation, &mut context);
            snapshots.push(context.clone());
            outcome
        })
        .collect();

    // Then: nesting preserves the same payload and effect boundaries.
    assert_eq!(
        outcomes,
        [
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Progressed),
            Ok(RunOutcome::Complete(Ok(14))),
        ]
    );
    assert_eq!(snapshots, [vec![], vec![11], vec![11], vec![11, 13]]);
}

#[test]
fn nested_preparation_preserves_every_handoff() {
    // Given: two preparation stages before an effect.
    let mut pipeline = Prepare::default()
        .then(Prepare::default())
        .then(Effect::default());
    let mut operation = Operation::new(10);
    let mut context = Vec::new();

    // When: both preparation steps finish.
    let outcomes: Vec<_> = (0..2)
        .map(|_| StepExecutor.step(&mut pipeline, &mut operation, &mut context))
        .collect();

    // Then: preparation completes without starting the effect.
    assert_eq!(
        outcomes,
        [Ok(RunOutcome::Progressed), Ok(RunOutcome::Progressed)]
    );
    assert_eq!(pipeline.upstream.upstream.calls, 1);
    assert_eq!(pipeline.upstream.downstream.calls, 1);
    assert_eq!(pipeline.downstream.calls, 0);
    assert!(context.is_empty());
}

#[test]
fn borrowed_stage_preserves_handoff_boundary() {
    // Given: a borrowed preparation stage before an effect.
    let mut prepare = Prepare::default();
    let mut pipeline = Then::new(&mut prepare, Effect::default());
    let mut operation = Operation::new(10);
    let mut context = Vec::new();

    // When: the executor advances through the borrowed stage.
    let outcome = StepExecutor.step(&mut pipeline, &mut operation, &mut context);

    // Then: borrowing preserves the boundary before the effect.
    assert_eq!(outcome, Ok(RunOutcome::Progressed));
    assert_eq!(pipeline.upstream.calls, 1);
    assert_eq!(pipeline.downstream.calls, 0);
    assert!(context.is_empty());
}

#[test]
fn two_effect_stages_keep_their_boundary() {
    // Given: two effect stages.
    let mut pipeline = Effect::default().then(Effect::default());
    let mut operation = Operation::new(10);
    let mut context = Vec::new();

    // When: the executor advances one step.
    let outcome = StepExecutor.step(&mut pipeline, &mut operation, &mut context);

    // Then: only upstream interacts.
    assert_eq!(outcome, Ok(RunOutcome::Progressed));
    assert_eq!(context, [10]);
    assert_eq!(pipeline.downstream.calls, 0);
}
