//! Consumer-visible tests for static pipeline composition.

use std::{cell::Cell, rc::Rc};

use hw_model_pipeline::{Operation, PipelineExt, RunError, Stage, StepOutcome, StepResult, Then};

#[derive(Debug, PartialEq, Eq)]
struct OwnedInput(String, Rc<Cell<usize>>);

#[derive(Debug, PartialEq, Eq)]
struct OwnedOutput(String);

#[derive(Debug, PartialEq, Eq)]
struct OwnedError(String);

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

    fn step(
        &mut self,
        _: &mut Context,
        state: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        state.take().map_or(Err(RunError::Exhausted), |value| {
            Ok(StepOutcome::Complete(value))
        })
    }
}

#[derive(Debug)]
struct Downstream {
    calls: Rc<Cell<usize>>,
    pending: Option<DownstreamState>,
}

#[derive(Debug)]
struct DownstreamState {
    pending_remaining: usize,
    input: OwnedInput,
}

impl Stage<Context> for Downstream {
    type Input = OwnedInput;
    type Error = OwnedError;
    type Output = OwnedOutput;

    fn step(
        &mut self,
        _: &mut Context,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.pending.is_none() {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            input.1.set(input.1.get() + 1);
            self.pending = Some(DownstreamState {
                pending_remaining: 100,
                input,
            });
        }
        let mut state = self.pending.take().ok_or(RunError::Exhausted)?;
        self.calls.set(self.calls.get() + 1);
        if state.pending_remaining == 0 {
            Ok(StepOutcome::Complete(Ok(OwnedOutput(format!(
                "{} -> downstream output",
                state.input.0
            )))))
        } else {
            state.pending_remaining -= 1;
            self.pending = Some(state);
            Ok(StepOutcome::InProgress)
        }
    }
}

#[test]
fn downstream_pending_starts_once_without_replaying_upstream() {
    // Given: reusable upstream and downstream hardware with 100 unfinished steps per request.
    let upstream_calls = Rc::new(Cell::new(0));
    let starts = Rc::new(Cell::new(0));
    let downstream_calls = Rc::new(Cell::new(0));
    let upstream = Upstream {
        calls: Rc::clone(&upstream_calls),
    };
    let mut pipeline: Then<Upstream, Downstream, OwnedInput> = upstream.then(Downstream {
        calls: Rc::clone(&downstream_calls),
        pending: None,
    });
    let mut operation = Operation::new(Ok(OwnedInput(
        "upstream input".to_owned(),
        Rc::clone(&starts),
    )));
    let mut context = Context;

    // When: the consumer transitions, discards unfinished outcomes, and resumes to completion.
    let transition = operation.step(&mut pipeline, &mut context);

    // Then: buffering the downstream offer is a boundary without starting downstream hardware.
    assert_eq!(transition, Ok(StepOutcome::InProgress));
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(starts.get(), 0);
    assert_eq!(downstream_calls.get(), 0);
    println!(
        "transition upstream={} starts={} downstream={}",
        upstream_calls.get(),
        starts.get(),
        downstream_calls.get()
    );

    for _ in 0..100 {
        let pending = operation.step(&mut pipeline, &mut context);
        assert_eq!(pending, Ok(StepOutcome::InProgress));
        drop(pending);
    }

    let complete = operation.step(&mut pipeline, &mut context);
    assert_eq!(
        complete,
        Ok(StepOutcome::Complete(Ok(OwnedOutput(
            "upstream input -> downstream output".to_owned()
        ))))
    );
    assert_eq!(
        operation.step(&mut pipeline, &mut context),
        Err(RunError::Exhausted)
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
fn adverse_upstream_error_skips_downstream_and_remains_exhausted() {
    // Given: an upstream error and persistent downstream hardware with observable calls.
    let upstream_calls = Rc::new(Cell::new(0));
    let downstream_calls = Rc::new(Cell::new(0));
    let upstream = Upstream {
        calls: Rc::clone(&upstream_calls),
    };
    let mut pipeline = upstream.then(Downstream {
        calls: Rc::clone(&downstream_calls),
        pending: None,
    });
    let mut operation = Operation::new(Err(OwnedError("upstream error".to_owned())));
    let mut context = Context;

    // When: the consumer observes the upstream error and steps the terminal pipeline again.
    let error = operation.step(&mut pipeline, &mut context);

    // Then: downstream is never polled, and terminal operation steps are inert.
    assert_eq!(
        error,
        Ok(StepOutcome::Complete(Err(OwnedError(
            "upstream error".to_owned()
        ))))
    );
    assert_eq!(
        operation.step(&mut pipeline, &mut context),
        Err(RunError::Exhausted)
    );
    assert_eq!(
        operation.step(&mut pipeline, &mut context),
        Err(RunError::Exhausted)
    );
    assert_eq!(upstream_calls.get(), 1);
    assert_eq!(downstream_calls.get(), 0);
    println!(
        "adverse upstream={} downstream={} post_terminal=Exhausted",
        upstream_calls.get(),
        downstream_calls.get()
    );
}
