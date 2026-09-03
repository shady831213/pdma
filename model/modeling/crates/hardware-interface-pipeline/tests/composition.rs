//! Consumer-visible tests for static pipeline composition.

use std::{cell::Cell, rc::Rc};

use hardware_interface_pipeline::{PipelineExt, Stage, StepOutcome, Then};

#[derive(Debug, PartialEq, Eq)]
struct OwnedInput(String);

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
    result: Option<Result<OwnedInput, OwnedError>>,
}

impl Upstream {
    fn succeeding(calls: Rc<Cell<usize>>) -> Self {
        Self {
            calls,
            result: Some(Ok(OwnedInput("upstream input".to_owned()))),
        }
    }

    fn failing(calls: Rc<Cell<usize>>) -> Self {
        Self {
            calls,
            result: Some(Err(OwnedError("upstream error".to_owned()))),
        }
    }
}

impl Stage<Context> for Upstream {
    type Error = OwnedError;
    type Output = OwnedInput;
    type Wait = OwnedWait;

    fn step(&mut self, _: &mut Context) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.calls.set(self.calls.get() + 1);
        self.result
            .take()
            .map_or(StepOutcome::Exhausted, StepOutcome::Complete)
    }
}

#[derive(Debug)]
struct Downstream {
    calls: Rc<Cell<usize>>,
    pending_remaining: usize,
    input: OwnedInput,
}

impl Downstream {
    const fn new(calls: Rc<Cell<usize>>, input: OwnedInput) -> Self {
        Self {
            calls,
            pending_remaining: 100,
            input,
        }
    }
}

impl Stage<Context> for Downstream {
    type Error = OwnedError;
    type Output = OwnedOutput;
    type Wait = OwnedWait;

    fn step(&mut self, _: &mut Context) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.calls.set(self.calls.get() + 1);
        if self.pending_remaining == 0 {
            StepOutcome::Complete(Ok(OwnedOutput(format!(
                "{} -> downstream output",
                self.input.0
            ))))
        } else {
            self.pending_remaining -= 1;
            StepOutcome::Pending(OwnedWait("owned pending token".to_owned()))
        }
    }
}

#[test]
fn downstream_pending_constructs_once_without_replaying_upstream() {
    // Given: an upstream output, a one-shot binder, and a downstream with 100 owned waits.
    let upstream_calls = Rc::new(Cell::new(0));
    let binder_calls = Rc::new(Cell::new(0));
    let downstream_calls = Rc::new(Cell::new(0));
    let binder_calls_for_stage = Rc::clone(&binder_calls);
    let downstream_calls_for_stage = Rc::clone(&downstream_calls);
    let upstream = Upstream::succeeding(Rc::clone(&upstream_calls));
    let mut pipeline: Then<Upstream, _, Downstream> = upstream.then(move |input| {
        binder_calls_for_stage.set(binder_calls_for_stage.get() + 1);
        Downstream::new(Rc::clone(&downstream_calls_for_stage), input)
    });
    let mut context = Context;

    // When: the consumer transitions, cancels each pending token, and resumes to completion.
    let transition = pipeline.step(&mut context);

    // Then: construction is a boundary and must not poll the newly installed downstream.
    assert_eq!(transition, StepOutcome::Progressed);
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(binder_calls.get(), 1);
    assert_eq!(downstream_calls.get(), 0);
    println!(
        "transition upstream={} binder={} downstream={}",
        upstream_calls.get(),
        binder_calls.get(),
        downstream_calls.get()
    );

    for _ in 0..100 {
        let pending = pipeline.step(&mut context);
        assert_eq!(
            pending,
            StepOutcome::Pending(OwnedWait("owned pending token".to_owned()))
        );
        drop(pending);
    }

    let complete = pipeline.step(&mut context);
    assert_eq!(
        complete,
        StepOutcome::Complete(Ok(OwnedOutput(
            "upstream input -> downstream output".to_owned()
        )))
    );
    assert_eq!(pipeline.step(&mut context), StepOutcome::Exhausted);
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(binder_calls.get(), 1);
    assert_eq!(downstream_calls.get(), 101);
    println!(
        "upstream={} binder={} downstream={} pending=100 owned_output=upstream input -> downstream output",
        upstream_calls.get(),
        binder_calls.get(),
        downstream_calls.get()
    );
}

#[test]
fn adverse_upstream_error_skips_downstream_and_remains_exhausted() {
    // Given: an upstream error and a binder whose construction would be externally observable.
    let upstream_calls = Rc::new(Cell::new(0));
    let binder_calls = Rc::new(Cell::new(0));
    let downstream_calls = Rc::new(Cell::new(0));
    let binder_calls_for_stage = Rc::clone(&binder_calls);
    let downstream_calls_for_stage = Rc::clone(&downstream_calls);
    let upstream = Upstream::failing(Rc::clone(&upstream_calls));
    let mut pipeline = upstream.then(move |input| {
        binder_calls_for_stage.set(binder_calls_for_stage.get() + 1);
        Downstream::new(Rc::clone(&downstream_calls_for_stage), input)
    });
    let mut context = Context;

    // When: the consumer observes the upstream error and steps the terminal pipeline again.
    let error = pipeline.step(&mut context);

    // Then: downstream is neither constructed nor polled, and terminal steps are inert.
    assert_eq!(
        error,
        StepOutcome::Complete(Err(OwnedError("upstream error".to_owned())))
    );
    assert_eq!(pipeline.step(&mut context), StepOutcome::Exhausted);
    assert_eq!(pipeline.step(&mut context), StepOutcome::Exhausted);
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(binder_calls.get(), 0);
    assert_eq!(downstream_calls.get(), 0);
    println!(
        "adverse upstream={} binder={} downstream={} post_terminal=Exhausted",
        upstream_calls.get(),
        binder_calls.get(),
        downstream_calls.get()
    );
}
