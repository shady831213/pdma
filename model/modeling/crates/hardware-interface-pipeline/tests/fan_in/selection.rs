use super::policy::Policy;
use hardware_interface_pipeline::{Arbitrate, Operation, ReadyStage, StepOutcome};

use super::sources::{Context, Job, Queue};

#[test]
fn selection_consumes_only_the_ready_granted_source() {
    // Given: one empty source followed by two populated queues.
    let mut context = Context::default();
    context.enqueue(1, [Job::Value(11)]);
    context.enqueue(2, [Job::Value(22)]);
    let mut hardware = Arbitrate::new(
        vec![Queue::new(0), Queue::new(1), Queue::new(2)],
        Policy::round_robin("idle"),
    );
    let mut operation = Operation::new(());

    // When: each outer step probes one candidate until a grant.
    let first = operation.step(&mut hardware, &mut context);
    let probes_after_first = context.probes.borrow().clone();
    let granted = operation.step(&mut hardware, &mut context);

    // Then: the selected queue alone is consumed and later candidates remain untouched.
    assert_eq!(first, StepOutcome::Progressed);
    assert_eq!(probes_after_first, [0]);
    assert_eq!(granted, StepOutcome::Complete(Ok(11)));
    assert_eq!(context.consumed, [1]);
    assert_eq!(*context.probes.borrow(), [0, 1]);
    assert_eq!(
        context.queues.get(&2),
        Some(&Vec::from([Job::Value(22)]).into())
    );
}

#[test]
fn round_robin_wraps_across_repeated_operations() {
    // Given: three queues with two entries each.
    let mut context = Context::default();
    for id in 0..3 {
        context.enqueue(id, [Job::Value(id), Job::Value(id + 3)]);
    }
    let mut hardware = Arbitrate::new(
        (0..3).map(Queue::new).collect(),
        Policy::round_robin("idle"),
    );

    // When: six independent operations use the same hardware.
    let outcomes: Vec<_> = (0..6)
        .map(|_| Operation::new(()).step(&mut hardware, &mut context))
        .collect();

    // Then: every source gets a turn before any source is selected again.
    assert_eq!(
        outcomes,
        (0..6)
            .map(|value| StepOutcome::Complete(Ok(value)))
            .collect::<Vec<_>>()
    );
    assert_eq!(context.consumed, [0, 1, 2, 0, 1, 2]);
}

#[test]
fn fixed_priority_restarts_at_the_first_source() {
    // Given: the first source can serve two operations while a later source is ready.
    let mut context = Context::default();
    context.enqueue(0, [Job::Value(10), Job::Value(11)]);
    context.enqueue(1, [Job::Value(20)]);
    let mut hardware = Arbitrate::new(
        vec![Queue::new(0), Queue::new(1)],
        Policy::fixed_priority("idle"),
    );

    // When: two requests complete.
    let outcomes: Vec<_> = (0..2)
        .map(|_| Operation::new(()).step(&mut hardware, &mut context))
        .collect();

    // Then: both grants belong to the first source.
    assert_eq!(
        outcomes,
        [StepOutcome::Complete(Ok(10)), StepOutcome::Complete(Ok(11))]
    );
    assert_eq!(context.consumed, [0, 0]);
}

#[test]
fn idle_pass_returns_the_configured_wait_and_rechecks_sources() {
    // Given: two empty queues and a non-unit idle notification.
    let mut context = Context::default();
    let mut hardware = Arbitrate::new(
        vec![Queue::new(0), Queue::new(1)],
        Policy::round_robin("queue changed"),
    );
    let mut operation = Operation::new(());

    // When: a full empty pass is followed by newly available work.
    let first = operation.step(&mut hardware, &mut context);
    let idle = operation.step(&mut hardware, &mut context);
    context.enqueue(0, [Job::Value(7)]);
    let resumed = operation.step(&mut hardware, &mut context);

    // Then: idle suspends without consuming inputs and a later pass accepts the work.
    assert_eq!(first, StepOutcome::Progressed);
    assert_eq!(idle, StepOutcome::Pending("queue changed"));
    assert_eq!(resumed, StepOutcome::Complete(Ok(7)));
    assert_eq!(context.consumed, [0]);
}

#[test]
fn nested_arbiters_forward_readiness_without_consuming_queues() {
    // Given: two fan-in groups with only the second group ready.
    let mut context = Context::default();
    context.enqueue(2, [Job::Value(42)]);
    let groups = vec![
        Arbitrate::new(vec![Queue::new(0)], Policy::round_robin("idle")),
        Arbitrate::new(
            vec![Queue::new(1), Queue::new(2)],
            Policy::round_robin("idle"),
        ),
    ];
    let mut hardware = Arbitrate::new(groups, Policy::round_robin("idle"));
    let mut operation = Operation::new(());

    // When: readiness is observed through a borrowed compositor and the request is driven.
    assert!(ReadyStage::ready(&&mut hardware, &context));
    let consumed_after_readiness = context.consumed.clone();
    let outcomes: Vec<_> = (0..3)
        .map(|_| operation.step(&mut hardware, &mut context))
        .collect();

    // Then: readiness is non-consuming and nested selection delivers the only value.
    assert!(consumed_after_readiness.is_empty());
    assert_eq!(
        outcomes,
        [
            StepOutcome::Progressed,
            StepOutcome::Progressed,
            StepOutcome::Complete(Ok(42))
        ]
    );
    assert_eq!(context.consumed, [2]);
}
