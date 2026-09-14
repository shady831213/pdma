use super::policy::Policy;
use hardware_interface_pipeline::{Arbitrate, Operation, Stage, StepOutcome};

use super::sources::{Context, Job, Queue};

#[test]
fn invalidated_grant_reselects_before_first_child_step() {
    // Given: the selected queue loses readiness before its first interaction.
    let mut context = Context::default();
    context.enqueue(0, [Job::Value(8)]);
    context.enqueue(1, [Job::Value(9)]);
    let mut hardware = Arbitrate::new(
        vec![Queue::new(0), Queue::new(1)],
        Policy::round_robin("idle"),
    );
    let mut operation = Operation::new(());
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        StepOutcome::Progressed
    );
    context.queues.remove(&0);

    // When: the stale grant is stepped and the operation is driven through reselection.
    let invalidated = operation.step(&mut hardware, &mut context);
    let consumed_after_invalidation = context.consumed.clone();
    let selected = operation.step(&mut hardware, &mut context);
    let child = operation.step(&mut hardware, &mut context);
    let complete = operation.step(&mut hardware, &mut context);

    // Then: the stale source is never called and the ready source completes once.
    assert_eq!(invalidated, StepOutcome::Progressed);
    assert!(consumed_after_invalidation.is_empty());
    assert_eq!(selected, StepOutcome::Progressed);
    assert_eq!(child, StepOutcome::Progressed);
    assert_eq!(complete, StepOutcome::Complete(Ok(9)));
    assert_eq!(*context.probes.borrow(), [0, 0, 1, 1]);
    assert_eq!(context.consumed, [1]);
}

#[test]
fn pending_grant_retains_accepted_state_after_readiness_disappears() {
    // Given: the first queue accepts a value but completion requires release.
    let mut context = Context::default();
    context.enqueue(0, [Job::Value(8)]);
    context.enqueue(1, [Job::Value(9)]);
    let mut hardware = Arbitrate::new(
        vec![
            Queue {
                id: 0,
                delayed: true,
            },
            Queue::new(1),
        ],
        Policy::round_robin("idle"),
    );
    let mut operation = Operation::new(());

    // When: acceptance empties the source, two pending steps occur, then release arrives.
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        StepOutcome::Progressed
    );
    let accepted = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    let pending_again = operation.step(&mut hardware, &mut context);
    context.released = true;
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        StepOutcome::Progressed
    );
    let complete = operation.step(&mut hardware, &mut context);

    // Then: the grant never switches or probes again; the retained value completes once.
    assert_eq!(accepted, StepOutcome::Progressed);
    assert_eq!(pending, StepOutcome::Pending("source release"));
    assert_eq!(pending_again, pending);
    assert_eq!(complete, StepOutcome::Complete(Ok(8)));
    assert_eq!(*context.probes.borrow(), [0, 0]);
    assert_eq!(context.consumed, [0]);
}

#[test]
fn terminal_outcomes_are_forwarded_and_direct_stage_state_is_inert() {
    // Given: sources independently completing, failing, or exhausting immediately.
    for (job, expected) in [
        (Job::Value(3), StepOutcome::Complete(Ok(3))),
        (
            Job::Fail,
            StepOutcome::Complete(Err(hardware_interface_pipeline::BranchError::Application(
                "source fault",
            ))),
        ),
        (Job::End, StepOutcome::Exhausted),
    ] {
        let mut context = Context::default();
        context.enqueue(0, [job, Job::Value(4)]);
        let mut hardware = Arbitrate::new(vec![Queue::new(0)], Policy::round_robin("idle"));
        let mut state = <Arbitrate<Queue, Policy> as Stage<Context>>::start(());

        // When: the same state is stepped beyond its terminal result.
        assert_eq!(
            hardware.step(&mut context, &mut state),
            StepOutcome::Progressed
        );
        let first = match hardware.step(&mut context, &mut state) {
            StepOutcome::Progressed => hardware.step(&mut context, &mut state),
            outcome => outcome,
        };
        let second = hardware.step(&mut context, &mut state);

        // Then: exactly one input was consumed and the second step is inert.
        assert_eq!(first, expected);
        assert_eq!(second, StepOutcome::Exhausted);
        assert_eq!(context.consumed, [0]);
    }
}

#[test]
fn empty_arbiter_is_exhausted_without_interaction() {
    // Given: fan-in hardware with no sources.
    let mut hardware = Arbitrate::<Queue, _>::new(Vec::new(), Policy::round_robin("idle"));
    let mut context = Context::default();

    // When: an operation is driven once.
    let outcome = Operation::new(()).step(&mut hardware, &mut context);

    // Then: it is terminal without a wait or source consumption.
    assert_eq!(outcome, StepOutcome::Exhausted);
    assert!(context.consumed.is_empty());
}
