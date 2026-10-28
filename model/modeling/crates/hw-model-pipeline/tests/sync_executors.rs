//! Consumer-visible tests for deterministic synchronous pipeline executors.

use std::{cell::Cell, num::NonZeroUsize, num::TryFromIntError, rc::Rc};

use hw_model_pipeline::{
    ImmediateExecutor, Operation, PipelineExt, RunError, Stage, StepExecutor, StepOutcome,
    StepResult,
};

#[derive(Debug, Default, PartialEq, Eq)]
struct Context {
    actions: Vec<&'static str>,
}

#[derive(Debug, PartialEq, Eq)]
struct Value(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct Fault(&'static str);

#[derive(Debug)]
enum Behavior {
    Ready(Result<&'static str, &'static str>),
    Unfinished {
        completion: Result<&'static str, &'static str>,
    },
    Progressing(Rc<Cell<usize>>),
}

#[derive(Debug)]
struct Leaf {
    name: &'static str,
    behavior: Behavior,
    accepted: bool,
}

impl Leaf {
    const fn ready(name: &'static str, result: Result<&'static str, &'static str>) -> Self {
        Self {
            name,
            behavior: Behavior::Ready(result),
            accepted: false,
        }
    }

    const fn unfinished(
        name: &'static str,
        completion: Result<&'static str, &'static str>,
    ) -> Self {
        Self {
            name,
            behavior: Behavior::Unfinished { completion },
            accepted: false,
        }
    }

    const fn progressing(calls: Rc<Cell<usize>>) -> Self {
        Self {
            name: "progress",
            behavior: Behavior::Progressing(calls),
            accepted: false,
        }
    }
}

impl Stage<Context> for Leaf {
    type Input = Value;
    type Error = Fault;
    type Output = Value;

    fn step(
        &mut self,
        context: &mut Context,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let continuing = self.accepted;
        if !continuing {
            let Some(_) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.accepted = true;
        }
        match &self.behavior {
            Behavior::Ready(result) => {
                self.accepted = false;
                context.actions.push(self.name);
                Ok(StepOutcome::Complete(result.map(Value).map_err(Fault)))
            }
            Behavior::Unfinished { completion } => {
                if continuing {
                    self.accepted = false;
                    context.actions.push(self.name);
                    Ok(StepOutcome::Complete(completion.map(Value).map_err(Fault)))
                } else {
                    Ok(StepOutcome::InProgress)
                }
            }
            Behavior::Progressing(calls) => {
                calls.set(calls.get() + 1);
                Ok(StepOutcome::InProgress)
            }
        }
    }
}

fn ready_chain() -> impl Stage<Context, Input = Value, Output = Value, Error = Fault> {
    Leaf::ready("A", Ok("a"))
        .then(Leaf::ready("B", Ok("b")))
        .then(Leaf::ready("C", Ok("terminal")))
}

fn budget(value: usize) -> Result<NonZeroUsize, TryFromIntError> {
    NonZeroUsize::try_from(value)
}

#[test]
fn stepped_chain_calls_each_ready_leaf_once_in_three_steps() {
    // Given: three immediately-ready leaves statically composed into one pipeline.
    let executor = StepExecutor::new();
    let mut pipeline = ready_chain();
    let mut operation = Operation::new(Value("request"));
    let mut context = Context::default();

    // When: the consumer advances exactly three explicit executor steps.
    let first = executor.step(&mut pipeline, &mut operation, &mut context);
    let second = executor.step(&mut pipeline, &mut operation, &mut context);
    let third = executor.step(&mut pipeline, &mut operation, &mut context);

    // Then: composition transitions are not additional leaf actions.
    assert_eq!(first, Ok(StepOutcome::InProgress));
    assert_eq!(second, Ok(StepOutcome::InProgress));
    assert_eq!(third, Ok(StepOutcome::Complete(Ok(Value("terminal")))));
    assert_eq!(context.actions, ["A", "B", "C"]);
    println!(
        "stepped calls=3 actions={:?} output=terminal",
        context.actions
    );
}

#[test]
fn parity_ready_chain_has_the_same_terminal_value_as_stepping() -> Result<(), TryFromIntError> {
    // Given: separate equivalent ready chains for both drive policies.
    let stepped = StepExecutor::new();
    let immediate = ImmediateExecutor::new(budget(3)?);
    let mut stepped_hw = ready_chain();
    let mut immediate_hw = ready_chain();
    let mut stepped_op = Operation::new(Value("request"));
    let mut immediate_op = Operation::new(Value("request"));
    let mut stepped_cx = Context::default();
    let mut immediate_cx = Context::default();

    // When: one policy advances three times and the other runs with a per-run budget of three.
    let _ = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let _ = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let stepped_result = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let immediate_result = immediate.run(&mut immediate_hw, &mut immediate_op, &mut immediate_cx);

    // Then: both preserve the same owned terminal result and leaf action sequence.
    assert_eq!(
        stepped_result,
        Ok(StepOutcome::Complete(Ok(Value("terminal"))))
    );
    assert_eq!(
        immediate_result,
        Ok(StepOutcome::Complete(Ok(Value("terminal"))))
    );
    assert_eq!(stepped_cx.actions, ["A", "B", "C"]);
    assert_eq!(immediate_cx.actions, ["A", "B", "C"]);
    println!("parity ready stepped={stepped_result:?} immediate={immediate_result:?}");
    Ok(())
}

#[test]
fn unfinished_work_resumes_after_stepping_or_budget_exhaustion() -> Result<(), TryFromIntError> {
    // Given: equivalent stages retaining unfinished work before completion.
    let stepped = StepExecutor::new();
    let immediate = ImmediateExecutor::new(budget(1)?);
    let mut stepped_hw = Leaf::unfinished("P", Ok("done"));
    let mut immediate_hw = Leaf::unfinished("P", Ok("done"));
    let mut stepped_op = Operation::new(Value("request"));
    let mut immediate_op = Operation::new(Value("request"));
    let mut stepped_cx = Context::default();
    let mut immediate_cx = Context::default();

    // When: each policy consumes one step before a separate resume call.
    let stepped_pending = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let immediate_pending = immediate.run(&mut immediate_hw, &mut immediate_op, &mut immediate_cx);

    // Then: both policies retain the invocation for the later resume call.
    assert_eq!(stepped_pending, Ok(StepOutcome::InProgress));
    assert_eq!(immediate_pending, Err(RunError::BudgetExhausted));
    println!(
        "parity pending stepped_wait={stepped_pending:?} immediate_wait={immediate_pending:?}"
    );
    drop(stepped_pending);
    drop(immediate_pending);
    let stepped_result = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let immediate_result = immediate.run(&mut immediate_hw, &mut immediate_op, &mut immediate_cx);

    // Then: no action replays, and both resumed pipelines complete alike.
    assert_eq!(stepped_cx.actions, ["P"]);
    assert_eq!(immediate_cx.actions, ["P"]);
    assert_eq!(stepped_result, Ok(StepOutcome::Complete(Ok(Value("done")))));
    assert_eq!(
        immediate_result,
        Ok(StepOutcome::Complete(Ok(Value("done"))))
    );
    println!("parity pending stepped={stepped_result:?} immediate={immediate_result:?}");
    Ok(())
}

#[test]
fn parity_stage_error_preserves_owned_error() -> Result<(), TryFromIntError> {
    // Given: equivalent ready stages with an owned terminal error.
    let stepped = StepExecutor::new();
    let immediate = ImmediateExecutor::new(budget(1)?);
    let mut stepped_hw = Leaf::ready("E", Err("stage error"));
    let mut immediate_hw = Leaf::ready("E", Err("stage error"));
    let mut stepped_op = Operation::new(Value("request"));
    let mut immediate_op = Operation::new(Value("request"));
    let mut stepped_cx = Context::default();
    let mut immediate_cx = Context::default();

    // When: each policy drives its stage once.
    let stepped_result = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let immediate_result = immediate.run(&mut immediate_hw, &mut immediate_op, &mut immediate_cx);

    // Then: the stage error remains an owned terminal result rather than an executor error.
    assert_eq!(
        stepped_result,
        Ok(StepOutcome::Complete(Err(Fault("stage error"))))
    );
    assert_eq!(
        immediate_result,
        Ok(StepOutcome::Complete(Err(Fault("stage error"))))
    );
    println!("parity error stepped={stepped_result:?} immediate={immediate_result:?}");
    Ok(())
}

#[test]
fn immediate_budget_is_nonzero_per_run_and_leaves_progressing_stage_resumable()
-> Result<(), TryFromIntError> {
    // Given: an always-progressing stage and a nonzero budget that resets for each run.
    let calls = Rc::new(Cell::new(0));
    let mut pipeline = Leaf::progressing(Rc::clone(&calls));
    let mut operation = Operation::new(Value("request"));
    let executor = ImmediateExecutor::new(budget(3)?);
    let mut context = Context::default();

    // When: the consumer invokes the bounded runner twice.
    let first = executor.run(&mut pipeline, &mut operation, &mut context);

    // Then: each run is bounded exactly at its configured count and can be resumed.
    assert_eq!(first, Err(RunError::BudgetExhausted));
    let first_calls = calls.get();
    assert_eq!(first_calls, 3);
    println!("budget first_calls={first_calls} first={first:?}");
    let second = executor.run(&mut pipeline, &mut operation, &mut context);
    assert_eq!(second, Err(RunError::BudgetExhausted));
    assert_eq!(calls.get(), 6);
    let zero_budget = NonZeroUsize::new(0);
    assert_eq!(zero_budget, None);
    println!("budget resumed_calls={} second={second:?}", calls.get());
    println!("budget zero_rejected={}", zero_budget.is_none());
    Ok(())
}

#[test]
fn executors_report_typed_exhaustion_after_terminal_completion() -> Result<(), TryFromIntError> {
    // Given: terminal-ready stages for both drive policies.
    let stepped = StepExecutor::new();
    let immediate = ImmediateExecutor::new(budget(1)?);
    let mut stepped_hw = Leaf::ready("S", Ok("done"));
    let mut immediate_hw = Leaf::ready("I", Ok("done"));
    let mut stepped_op = Operation::new(Value("request"));
    let mut immediate_op = Operation::new(Value("request"));
    let mut stepped_cx = Context::default();
    let mut immediate_cx = Context::default();

    // When: each completed pipeline is driven once more.
    let _ = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let _ = immediate.run(&mut immediate_hw, &mut immediate_op, &mut immediate_cx);
    let stepped_terminal = stepped.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
    let immediate_terminal = immediate.run(&mut immediate_hw, &mut immediate_op, &mut immediate_cx);

    // Then: terminal exhaustion is a typed executor error with no further action.
    assert_eq!(stepped_terminal, Err(RunError::Exhausted));
    assert_eq!(immediate_terminal, Err(RunError::Exhausted));
    assert_eq!(stepped_cx.actions, ["S"]);
    assert_eq!(immediate_cx.actions, ["I"]);
    println!("post_terminal stepped={stepped_terminal:?} immediate={immediate_terminal:?}");
    Ok(())
}
