use super::policy::Policy;
use std::convert::Infallible;

use hardware_interface_pipeline::{AfterEach, Arbitrate, Operation, Stage, StepOutcome, Then};

use super::sources::{Context, Job, Queue};

struct Transform(u8);

impl Stage<Context> for Transform {
    type Input = u8;
    type Output = u8;
    type Error = &'static str;
    type Wait = &'static str;
    type State = Option<u8>;

    fn start(input: u8) -> Self::State {
        Some(input)
    }

    fn step(
        &mut self,
        context: &mut Context,
        state: &mut Self::State,
    ) -> StepOutcome<u8, Self::Error, Self::Wait> {
        let Some(value) = state.take() else {
            return StepOutcome::Exhausted;
        };
        context.transformed.push(self.0);
        StepOutcome::Complete(Ok(value + 10))
    }
}

struct Update(u8);

impl Stage<Context> for Update {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        context: &mut Context,
        (): &mut (),
    ) -> StepOutcome<Infallible, Infallible, Infallible> {
        context.updated.push(self.0);
        StepOutcome::Progressed
    }
}

#[test]
fn then_sources_gate_acceptance_and_retain_pending_upstream_and_downstream() {
    // Given: an empty composed source, a delayed source, and an unchosen ready source.
    let mut context = Context::default();
    context.enqueue(1, [Job::Value(7)]);
    context.enqueue(2, [Job::Value(8)]);
    let sources = (0..3)
        .map(|id| Then::new(Queue { id, delayed: true }, Transform(id)))
        .collect();
    let mut hardware = Arbitrate::new(sources, Policy::round_robin("idle"));
    let mut operation = Operation::new(());

    // When: the selected upstream suspends after acceptance and later hands off its value.
    let searching = operation.step(&mut hardware, &mut context);
    let accepted = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    context.released = true;
    let handoff = operation.step(&mut hardware, &mut context);
    let complete = operation.step(&mut hardware, &mut context);

    // Then: only the granted chain runs, retaining the value until its transform completes.
    assert_eq!(searching, StepOutcome::Progressed);
    assert_eq!(accepted, StepOutcome::Progressed);
    assert_eq!(pending, StepOutcome::Pending("source release"));
    assert_eq!(handoff, StepOutcome::Progressed);
    assert_eq!(complete, StepOutcome::Complete(Ok(17)));
    assert_eq!(*context.probes.borrow(), [0, 1]);
    assert_eq!(context.consumed, [1]);
    assert_eq!(context.transformed, [1]);
}

#[test]
fn after_each_sources_update_only_the_granted_composed_source() {
    // Given: one empty source followed by two ready chains with per-step updates.
    let mut context = Context::default();
    context.enqueue(1, [Job::Value(7)]);
    context.enqueue(2, [Job::Value(8)]);
    let sources = (0..3)
        .map(|id| {
            let chain = Then::new(Queue { id, delayed: true }, Transform(id));
            AfterEach::new(chain, Update(id))
        })
        .collect();
    let mut hardware = Arbitrate::new(sources, Policy::fixed_priority("idle"));
    let mut operation = Operation::new(());

    // When: selection skips the empty source, then resumes the granted chain after a wait.
    let searching = operation.step(&mut hardware, &mut context);
    let updates_after_search = context.updated.clone();
    let accepted = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    context.released = true;
    let handoff = operation.step(&mut hardware, &mut context);
    let complete = operation.step(&mut hardware, &mut context);

    // Then: readiness performs no update and each granted interaction runs its update once.
    assert_eq!(searching, StepOutcome::Progressed);
    assert!(updates_after_search.is_empty());
    assert_eq!(accepted, StepOutcome::Progressed);
    assert_eq!(pending, StepOutcome::Pending("source release"));
    assert_eq!(handoff, StepOutcome::Progressed);
    assert_eq!(complete, StepOutcome::Complete(Ok(17)));
    assert_eq!(*context.probes.borrow(), [0, 1]);
    assert_eq!(context.consumed, [1]);
    assert_eq!(context.transformed, [1]);
    assert_eq!(context.updated, [1, 1, 1, 1]);
}
