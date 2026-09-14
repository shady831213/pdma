use hardware_interface_pipeline::{Combiner, Operation, Selection, Stage, StepOutcome};

use super::sources::{Context, Job, Queue};

#[test]
fn pending_grant_retains_accepted_state_after_readiness_disappears() {
    // Given: the first queue accepts a value but completion requires release.
    let mut context = Context::default();
    context.enqueue(0, [Job::Value(8)]);
    context.enqueue(1, [Job::Value(9)]);
    let mut hardware = Combiner::new(
        vec![
            Queue {
                id: 0,
                delayed: true,
            },
            Queue::new(1),
        ],
        Selection::RoundRobin,
        "idle",
    );
    let mut operation = Operation::new(());

    // When: acceptance empties the source, two pending steps occur, then release arrives.
    let accepted = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    let pending_again = operation.step(&mut hardware, &mut context);
    context.released = true;
    let complete = operation.step(&mut hardware, &mut context);

    // Then: the grant never switches or probes again; the retained value completes once.
    assert_eq!(accepted, StepOutcome::Progressed);
    assert_eq!(pending, StepOutcome::Pending("source release"));
    assert_eq!(pending_again, pending);
    assert_eq!(complete, StepOutcome::Complete(Ok(8)));
    assert_eq!(*context.probes.borrow(), [0]);
    assert_eq!(context.consumed, [0]);
}

#[test]
fn terminal_outcomes_are_forwarded_and_direct_stage_state_is_inert() {
    // Given: sources independently completing, failing, or exhausting immediately.
    for (job, expected) in [
        (Job::Value(3), StepOutcome::Complete(Ok(3))),
        (Job::Fail, StepOutcome::Complete(Err("source fault"))),
        (Job::End, StepOutcome::Exhausted),
    ] {
        let mut context = Context::default();
        context.enqueue(0, [job, Job::Value(4)]);
        let mut hardware = Combiner::new(vec![Queue::new(0)], Selection::RoundRobin, "idle");
        let mut state = <Combiner<Queue, &str> as Stage<Context>>::start(());

        // When: the same state is stepped beyond its terminal result.
        let first = hardware.step(&mut context, &mut state);
        let second = hardware.step(&mut context, &mut state);

        // Then: exactly one input was consumed and the second step is inert.
        assert_eq!(first, expected);
        assert_eq!(second, StepOutcome::Exhausted);
        assert_eq!(context.consumed, [0]);
    }
}

#[test]
fn empty_combiner_is_exhausted_without_interaction() {
    // Given: fan-in hardware with no sources.
    let mut hardware = Combiner::<Queue, _>::new(Vec::new(), Selection::RoundRobin, "idle");
    let mut context = Context::default();

    // When: an operation is driven once.
    let outcome = Operation::new(()).step(&mut hardware, &mut context);

    // Then: it is terminal without a wait or source consumption.
    assert_eq!(outcome, StepOutcome::Exhausted);
    assert!(context.consumed.is_empty());
}
