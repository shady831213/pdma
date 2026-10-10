use super::policy::Policy;
use hardware_interface_pipeline::{Arbitrate, Operation, RunOutcome};

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
        Ok(RunOutcome::Progressed)
    );
    context.queues.remove(&0);

    // When: the stale grant is stepped and the operation is driven through reselection.
    let invalidated = operation.step(&mut hardware, &mut context);
    let consumed_after_invalidation = context.consumed.clone();
    let selected = operation.step(&mut hardware, &mut context);
    let child = operation.step(&mut hardware, &mut context);
    let complete = operation.step(&mut hardware, &mut context);

    // Then: the stale source is never called and the ready source completes once.
    assert_eq!(invalidated, Ok(RunOutcome::Progressed));
    assert!(consumed_after_invalidation.is_empty());
    assert_eq!(selected, Ok(RunOutcome::Progressed));
    assert_eq!(child, Ok(RunOutcome::Progressed));
    assert_eq!(complete, Ok(RunOutcome::Complete(Ok(9))));
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
        Ok(RunOutcome::Progressed)
    );
    let accepted = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    let pending_again = operation.step(&mut hardware, &mut context);
    context.released = true;
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let complete = operation.step(&mut hardware, &mut context);

    // Then: the grant never switches or probes again; the retained value completes once.
    assert_eq!(accepted, Ok(RunOutcome::Progressed));
    assert_eq!(pending, Ok(RunOutcome::Pending("source release")));
    assert_eq!(pending_again, pending);
    assert_eq!(complete, Ok(RunOutcome::Complete(Ok(8))));
    assert_eq!(*context.probes.borrow(), [0, 0]);
    assert_eq!(context.consumed, [0]);
}

#[test]
fn terminal_outcomes_are_forwarded_and_completed_operation_is_inert() {
    // Given: sources independently completing, failing, or ending immediately.
    for (job, expected) in [
        (Job::Value(3), Ok(RunOutcome::Complete(Ok(3)))),
        (
            Job::Fail,
            Ok(RunOutcome::Complete(Err(
                hardware_interface_pipeline::BranchError::Application("source fault"),
            ))),
        ),
        (
            Job::End,
            Ok(RunOutcome::Complete(Err(
                hardware_interface_pipeline::BranchError::Application("source ended"),
            ))),
        ),
    ] {
        let mut context = Context::default();
        context.enqueue(0, [job, Job::Value(4)]);
        let mut hardware = Arbitrate::new(vec![Queue::new(0)], Policy::round_robin("idle"));
        let mut state = Operation::new(());

        // When: the same state is stepped beyond its terminal result.
        assert_eq!(
            state.step(&mut hardware, &mut context),
            Ok(RunOutcome::Progressed)
        );
        let first = match state.step(&mut hardware, &mut context) {
            Ok(RunOutcome::Progressed) => state.step(&mut hardware, &mut context),
            outcome => outcome,
        };
        let second = state.step(&mut hardware, &mut context);

        // Then: exactly one input was consumed and the second step is inert.
        assert_eq!(first, expected);
        assert_eq!(second, Err(hardware_interface_pipeline::AlreadyComplete));
        assert_eq!(context.consumed, [0]);
    }
}

#[test]
fn empty_arbiter_reports_empty_sources_without_interaction() {
    // Given: fan-in hardware with no sources.
    let mut hardware = Arbitrate::<Queue, _>::new(Vec::new(), Policy::round_robin("idle"));
    let mut context = Context::default();

    // When: an operation is driven once.
    let outcome = Operation::new(()).step(&mut hardware, &mut context);

    // Then: it is terminal without a wait or source consumption.
    assert_eq!(
        outcome,
        Ok(RunOutcome::Complete(Err(
            hardware_interface_pipeline::BranchError::EmptySources
        )))
    );
    assert!(context.consumed.is_empty());
}
