//! Consumer-visible tests for static pipeline composition.

use std::{cell::Cell, rc::Rc};

use hardware_interface_pipeline::{Operation, PipelineExt, RunOutcome, Stage, StepOutcome, Then};

#[derive(Debug, PartialEq, Eq)]
struct OwnedInput(String, Rc<Cell<usize>>);

#[derive(Debug, PartialEq, Eq)]
struct OwnedOutput(String);

#[derive(Debug, PartialEq, Eq)]
struct OwnedError(String);

#[derive(Debug, PartialEq, Eq)]
struct OwnedWait(String);

#[derive(Debug, Default)]
struct Context;

#[derive(Debug)]
struct Upstream {
    calls: Rc<Cell<usize>>,
}

impl Stage<Context> for Upstream {
    type Input = Result<OwnedInput, OwnedError>;
    type Error = OwnedError;
    type Output = OwnedInput;
    type Wait = OwnedWait;
    type State = Self::Input;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        _: &mut Context,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        self.calls.set(self.calls.get() + 1);
        StepOutcome::Complete(state)
    }
}

#[derive(Debug)]
struct Downstream {
    calls: Rc<Cell<usize>>,
}

struct DownstreamState {
    pending_remaining: usize,
    input: OwnedInput,
}

impl Stage<Context> for Downstream {
    type Input = OwnedInput;
    type Error = OwnedError;
    type Output = OwnedOutput;
    type Wait = OwnedWait;
    type State = DownstreamState;

    fn start(input: Self::Input) -> Self::State {
        input.1.set(input.1.get() + 1);
        DownstreamState {
            pending_remaining: 100,
            input,
        }
    }

    fn step(
        &mut self,
        _: &mut Context,
        mut state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        self.calls.set(self.calls.get() + 1);
        if state.pending_remaining == 0 {
            StepOutcome::Complete(Ok(OwnedOutput(format!(
                "{} -> downstream output",
                state.input.0
            ))))
        } else {
            state.pending_remaining -= 1;
            StepOutcome::Pending(state, OwnedWait("owned pending token".to_owned()))
        }
    }
}

#[test]
fn downstream_pending_starts_once_without_replaying_upstream() {
    // Given: reusable upstream and downstream hardware with 100 owned waits per request.
    let upstream_calls = Rc::new(Cell::new(0));
    let starts = Rc::new(Cell::new(0));
    let downstream_calls = Rc::new(Cell::new(0));
    let upstream = Upstream {
        calls: Rc::clone(&upstream_calls),
    };
    let mut pipeline: Then<Upstream, Downstream> = upstream.then(Downstream {
        calls: Rc::clone(&downstream_calls),
    });
    let mut operation = Operation::new(Ok(OwnedInput(
        "upstream input".to_owned(),
        Rc::clone(&starts),
    )));
    let mut context = Context;

    // When: the consumer transitions, cancels each pending token, and resumes to completion.
    let transition = operation.step(&mut pipeline, &mut context);

    // Then: starting downstream state is a boundary and does not poll downstream hardware.
    assert_eq!(transition, Ok(RunOutcome::Progressed));
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(starts.get(), 1);
    assert_eq!(downstream_calls.get(), 0);
    println!(
        "transition upstream={} starts={} downstream={}",
        upstream_calls.get(),
        starts.get(),
        downstream_calls.get()
    );

    for _ in 0..100 {
        let pending = operation.step(&mut pipeline, &mut context);
        assert_eq!(
            pending,
            Ok(RunOutcome::Pending(OwnedWait(
                "owned pending token".to_owned()
            )))
        );
        drop(pending);
    }

    let complete = operation.step(&mut pipeline, &mut context);
    assert_eq!(
        complete,
        Ok(RunOutcome::Complete(Ok(OwnedOutput(
            "upstream input -> downstream output".to_owned()
        ))))
    );
    assert_eq!(
        operation.step(&mut pipeline, &mut context),
        Err(hardware_interface_pipeline::AlreadyComplete)
    );
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(starts.get(), 1);
    assert_eq!(downstream_calls.get(), 101);
    println!(
        "upstream={} starts={} downstream={} pending=100 owned_output=upstream input -> downstream output",
        upstream_calls.get(),
        starts.get(),
        downstream_calls.get()
    );
}

#[test]
fn adverse_upstream_error_skips_downstream_and_remains_complete() {
    // Given: an upstream error and persistent downstream hardware with observable calls.
    let upstream_calls = Rc::new(Cell::new(0));
    let downstream_calls = Rc::new(Cell::new(0));
    let upstream = Upstream {
        calls: Rc::clone(&upstream_calls),
    };
    let mut pipeline = upstream.then(Downstream {
        calls: Rc::clone(&downstream_calls),
    });
    let mut operation = Operation::new(Err(OwnedError("upstream error".to_owned())));
    let mut context = Context;

    // When: the consumer observes the upstream error and steps the terminal pipeline again.
    let error = operation.step(&mut pipeline, &mut context);

    // Then: downstream is never polled, and terminal operation steps are inert.
    assert_eq!(
        error,
        Ok(RunOutcome::Complete(Err(OwnedError(
            "upstream error".to_owned()
        ))))
    );
    assert_eq!(
        operation.step(&mut pipeline, &mut context),
        Err(hardware_interface_pipeline::AlreadyComplete)
    );
    assert_eq!(
        operation.step(&mut pipeline, &mut context),
        Err(hardware_interface_pipeline::AlreadyComplete)
    );
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(downstream_calls.get(), 0);
    println!(
        "adverse upstream={} downstream={} post_terminal=AlreadyComplete",
        upstream_calls.get(),
        downstream_calls.get()
    );
}
