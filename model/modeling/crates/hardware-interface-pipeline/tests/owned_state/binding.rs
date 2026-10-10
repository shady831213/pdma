use std::{
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use hardware_interface_pipeline::{
    OnContinue, Operation, Repeat, RunOutcome, Stage, StepOutcome, Then,
};

#[derive(Debug, PartialEq, Eq)]
struct Payload(String);

struct Bind;

impl Stage<(usize, bool, bool)> for Bind {
    type Input = (Arc<AtomicUsize>, Payload);
    type Output = ControlFlow<Payload, Payload>;
    type Error = u8;
    type Wait = ();
    type State = Payload;
    fn start((starts, payload): Self::Input) -> Self::State {
        starts.fetch_add(1, Ordering::SeqCst);
        payload
    }
    fn step(
        &mut self,
        context: &mut (usize, bool, bool),
        state: Payload,
    ) -> StepOutcome<Payload, Self::Output, u8, ()> {
        context.0 += 1;
        if context.2 {
            StepOutcome::Complete(Err(7))
        } else if context.1 {
            StepOutcome::Complete(Ok(ControlFlow::Continue(state)))
        } else {
            StepOutcome::Pending(state, ())
        }
    }
}

static BREAK_STARTS: AtomicUsize = AtomicUsize::new(0);
struct BreakChild;

impl Stage<usize> for BreakChild {
    type Input = Payload;
    type Output = ControlFlow<Payload, Payload>;
    type Error = u8;
    type Wait = ();
    type State = Payload;
    fn start(input: Payload) -> Payload {
        BREAK_STARTS.fetch_add(1, Ordering::SeqCst);
        input
    }
    fn step(
        &mut self,
        calls: &mut usize,
        state: Payload,
    ) -> StepOutcome<Payload, Self::Output, u8, ()> {
        *calls += 1;
        StepOutcome::Complete(Ok(ControlFlow::Continue(state)))
    }
}

#[test]
fn break_bypasses_child_start_and_step_with_nonclone_payload() {
    // Given a binding adapter and a breaking owned payload.
    let mut stage = OnContinue::new(BreakChild);
    let mut context = 0;
    let mut operation = Operation::new(ControlFlow::Break(Payload("stop".into())));
    // When the adapter is driven once.
    let result = operation.step(&mut stage, &mut context);
    // Then it returns the same payload without touching child hardware.
    assert_eq!(
        result,
        Ok(RunOutcome::Complete(Ok(ControlFlow::Break(Payload(
            "stop".into()
        )))))
    );
    assert_eq!(context, 0);
    assert_eq!(BREAK_STARTS.load(Ordering::SeqCst), 0);
}

#[test]
fn continuing_binding_resumes_exact_state_and_starts_once() {
    // Given a child whose first interaction waits.
    let starts = Arc::new(AtomicUsize::new(0));
    let mut stage = OnContinue::new(Bind);
    let mut context = (0, false, false);
    let mut operation = Operation::new(ControlFlow::Continue((
        Arc::clone(&starts),
        Payload("owned".into()),
    )));
    assert_eq!(
        operation.step(&mut stage, &mut context),
        Ok(RunOutcome::Pending(()))
    );
    context.1 = true;
    // When the same operation resumes.
    let result = operation.step(&mut stage, &mut context);
    // Then its payload survives and child initialization is not replayed.
    assert_eq!(
        result,
        Ok(RunOutcome::Complete(Ok(ControlFlow::Continue(Payload(
            "owned".into()
        )))))
    );
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(context.0, 2);
}

#[test]
fn binding_preserves_structural_child_error() {
    // Given a continuing child configured to fail structurally.
    let starts = Arc::new(AtomicUsize::new(0));
    let mut stage = OnContinue::new(Bind);
    let mut context = (0, true, true);
    let mut operation = Operation::new(ControlFlow::Continue((starts, Payload("error".into()))));
    // When the binding advances the child.
    let result = operation.step(&mut stage, &mut context);
    // Then its error remains structural and terminal.
    assert_eq!(result, Ok(RunOutcome::Complete(Err(7))));
    assert!(operation.is_complete());
}

struct Inner;
struct Outer;
struct Skipped;
type Frame = (u8, String);

impl Stage<usize> for Inner {
    type Input = Frame;
    type Output = ControlFlow<Frame, Frame>;
    type Error = u8;
    type Wait = ();
    type State = Frame;
    fn start(input: Frame) -> Frame {
        input
    }
    fn step(
        &mut self,
        _: &mut usize,
        (rounds, mut text): Frame,
    ) -> StepOutcome<Frame, Self::Output, u8, ()> {
        text.push('i');
        StepOutcome::Complete(Ok(ControlFlow::Break((rounds, text))))
    }
}

impl Stage<usize> for Skipped {
    type Input = Frame;
    type Output = ControlFlow<Frame, Frame>;
    type Error = u8;
    type Wait = ();
    type State = Frame;
    fn start(input: Frame) -> Frame {
        input
    }
    fn step(
        &mut self,
        calls: &mut usize,
        state: Frame,
    ) -> StepOutcome<Frame, Self::Output, u8, ()> {
        *calls += 1;
        StepOutcome::Complete(Ok(ControlFlow::Continue(state)))
    }
}

impl Stage<usize> for Outer {
    type Input = Frame;
    type Output = ControlFlow<Payload, Frame>;
    type Error = u8;
    type Wait = ();
    type State = Frame;
    fn start(input: Frame) -> Frame {
        input
    }
    fn step(
        &mut self,
        _: &mut usize,
        (rounds, mut text): Frame,
    ) -> StepOutcome<Frame, Self::Output, u8, ()> {
        text.push('o');
        StepOutcome::Complete(Ok(if rounds == 1 {
            ControlFlow::Break(Payload(text))
        } else {
            ControlFlow::Continue((rounds - 1, text))
        }))
    }
}

#[test]
fn inner_break_ends_only_inner_repeat() {
    // Given nested repeats and an explicit binding inside the inner loop.
    let inner = Repeat::new(Then::new(Inner, OnContinue::new(Skipped)));
    let mut stage = Repeat::new(Then::new(inner, Outer));
    let mut operation = Operation::new((2, String::new()));
    let mut calls = 0;
    let mut terminal = None;
    // When the outer operation is driven to its bounded completion.
    for _ in 0..20 {
        match operation
            .step(&mut stage, &mut calls)
            .expect("live operation")
        {
            RunOutcome::Progressed => {}
            RunOutcome::Pending(()) => break,
            RunOutcome::Complete(result) => {
                terminal = Some(result);
                break;
            }
        }
    }
    // Then both outer iterations run while each inner break skips its bound child.
    assert_eq!(terminal, Some(Ok(Payload("ioio".into()))));
    assert_eq!(calls, 0);
}
