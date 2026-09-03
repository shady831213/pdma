//! Consumer-visible tests for deterministic synchronous pipeline executors.

use std::{cell::Cell, num::NonZeroUsize, num::TryFromIntError, rc::Rc};

use hardware_interface_pipeline::{
    ImmediateExecutor, PipelineExt, RunError, RunOutcome, Stage, StepExecutor, StepOutcome,
};

#[derive(Debug, Default, PartialEq, Eq)]
struct Context {
    actions: Vec<&'static str>,
}

#[derive(Debug, PartialEq, Eq)]
struct Value(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct Fault(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct Wait(&'static str);

#[derive(Debug)]
enum LeafState {
    Ready(Option<Result<Value, Fault>>),
    Pending {
        wait: Option<Wait>,
        completion: Option<Result<Value, Fault>>,
    },
    Progressing(Rc<Cell<usize>>),
}

#[derive(Debug)]
struct Leaf {
    name: &'static str,
    state: LeafState,
}

impl Leaf {
    const fn ready(name: &'static str, result: Result<Value, Fault>) -> Self {
        Self {
            name,
            state: LeafState::Ready(Some(result)),
        }
    }

    const fn pending(name: &'static str, wait: Wait, completion: Result<Value, Fault>) -> Self {
        Self {
            name,
            state: LeafState::Pending {
                wait: Some(wait),
                completion: Some(completion),
            },
        }
    }

    const fn progressing(calls: Rc<Cell<usize>>) -> Self {
        Self {
            name: "progress",
            state: LeafState::Progressing(calls),
        }
    }
}

impl Stage<Context> for Leaf {
    type Error = Fault;
    type Output = Value;
    type Wait = Wait;

    fn step(
        &mut self,
        context: &mut Context,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        match &mut self.state {
            LeafState::Ready(result) => match result.take() {
                Some(result) => {
                    context.actions.push(self.name);
                    StepOutcome::Complete(result)
                }
                None => StepOutcome::Exhausted,
            },
            LeafState::Pending { wait, completion } => match wait.take() {
                Some(wait) => StepOutcome::Pending(wait),
                None => match completion.take() {
                    Some(result) => {
                        context.actions.push(self.name);
                        StepOutcome::Complete(result)
                    }
                    None => StepOutcome::Exhausted,
                },
            },
            LeafState::Progressing(calls) => {
                calls.set(calls.get() + 1);
                StepOutcome::Progressed
            }
        }
    }
}

fn ready_chain() -> impl Stage<Context, Output = Value, Error = Fault, Wait = Wait> {
    Leaf::ready("A", Ok(Value("a")))
        .then(|_| Leaf::ready("B", Ok(Value("b"))))
        .then(|_| Leaf::ready("C", Ok(Value("terminal"))))
}

fn budget(value: usize) -> Result<NonZeroUsize, TryFromIntError> {
    NonZeroUsize::try_from(value)
}

#[test]
fn stepped_chain_calls_each_ready_leaf_once_in_three_steps() {
    // Given: three immediately-ready leaves statically composed into one pipeline.
    let executor = StepExecutor::new();
    let mut pipeline = ready_chain();
    let mut context = Context::default();

    // When: the consumer advances exactly three explicit executor steps.
    let first = executor.step(&mut pipeline, &mut context);
    let second = executor.step(&mut pipeline, &mut context);
    let third = executor.step(&mut pipeline, &mut context);

    // Then: composition transitions are not additional leaf actions.
    assert_eq!(first, Ok(RunOutcome::Progressed));
    assert_eq!(second, Ok(RunOutcome::Progressed));
    assert_eq!(third, Ok(RunOutcome::Complete(Ok(Value("terminal")))));
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
    let mut stepped_pipeline = ready_chain();
    let mut immediate_pipeline = ready_chain();
    let mut stepped_context = Context::default();
    let mut immediate_context = Context::default();

    // When: one policy advances three times and the other runs with a per-run budget of three.
    let _ = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let _ = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let stepped_result = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let immediate_result = immediate.run(&mut immediate_pipeline, &mut immediate_context);

    // Then: both preserve the same owned terminal result and leaf action sequence.
    assert_eq!(
        stepped_result,
        Ok(RunOutcome::Complete(Ok(Value("terminal"))))
    );
    assert_eq!(
        immediate_result,
        Ok(RunOutcome::Complete(Ok(Value("terminal"))))
    );
    assert_eq!(stepped_context.actions, ["A", "B", "C"]);
    assert_eq!(immediate_context.actions, ["A", "B", "C"]);
    println!("parity ready stepped={stepped_result:?} immediate={immediate_result:?}");
    Ok(())
}

#[test]
fn parity_pending_returns_owned_wait_then_resumes_without_replay() -> Result<(), TryFromIntError> {
    // Given: equivalent stages that first transfer an owned wait and then complete.
    let stepped = StepExecutor::new();
    let immediate = ImmediateExecutor::new(budget(1)?);
    let mut stepped_pipeline = Leaf::pending("P", Wait("wait"), Ok(Value("done")));
    let mut immediate_pipeline = Leaf::pending("P", Wait("wait"), Ok(Value("done")));
    let mut stepped_context = Context::default();
    let mut immediate_context = Context::default();

    // When: each policy returns and drops its wait before a separate resume call.
    let stepped_pending = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let immediate_pending = immediate.run(&mut immediate_pipeline, &mut immediate_context);

    // Then: both policies transfer the same owned wait before the later resume call.
    assert_eq!(stepped_pending, Ok(RunOutcome::Pending(Wait("wait"))));
    assert_eq!(immediate_pending, Ok(RunOutcome::Pending(Wait("wait"))));
    println!(
        "parity pending stepped_wait={stepped_pending:?} immediate_wait={immediate_pending:?}"
    );
    drop(stepped_pending);
    drop(immediate_pending);
    let stepped_result = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let immediate_result = immediate.run(&mut immediate_pipeline, &mut immediate_context);

    // Then: waits are owned, no action replays, and both resumed pipelines complete alike.
    assert_eq!(stepped_context.actions, ["P"]);
    assert_eq!(immediate_context.actions, ["P"]);
    assert_eq!(stepped_result, Ok(RunOutcome::Complete(Ok(Value("done")))));
    assert_eq!(
        immediate_result,
        Ok(RunOutcome::Complete(Ok(Value("done"))))
    );
    println!("parity pending stepped={stepped_result:?} immediate={immediate_result:?}");
    Ok(())
}

#[test]
fn parity_stage_error_preserves_owned_error() -> Result<(), TryFromIntError> {
    // Given: equivalent ready stages with an owned terminal error.
    let stepped = StepExecutor::new();
    let immediate = ImmediateExecutor::new(budget(1)?);
    let mut stepped_pipeline = Leaf::ready("E", Err(Fault("stage error")));
    let mut immediate_pipeline = Leaf::ready("E", Err(Fault("stage error")));
    let mut stepped_context = Context::default();
    let mut immediate_context = Context::default();

    // When: each policy drives its stage once.
    let stepped_result = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let immediate_result = immediate.run(&mut immediate_pipeline, &mut immediate_context);

    // Then: the stage error remains an owned terminal result rather than an executor error.
    assert_eq!(
        stepped_result,
        Ok(RunOutcome::Complete(Err(Fault("stage error"))))
    );
    assert_eq!(
        immediate_result,
        Ok(RunOutcome::Complete(Err(Fault("stage error"))))
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
    let executor = ImmediateExecutor::new(budget(3)?);
    let mut context = Context::default();

    // When: the consumer invokes the bounded runner twice.
    let first = executor.run(&mut pipeline, &mut context);

    // Then: each run is bounded exactly at its configured count and can be resumed.
    assert_eq!(first, Err(RunError::BudgetExhausted));
    let first_calls = calls.get();
    assert_eq!(first_calls, 3);
    println!("budget first_calls={first_calls} first={first:?}");
    let second = executor.run(&mut pipeline, &mut context);
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
    let mut stepped_pipeline = Leaf::ready("S", Ok(Value("done")));
    let mut immediate_pipeline = Leaf::ready("I", Ok(Value("done")));
    let mut stepped_context = Context::default();
    let mut immediate_context = Context::default();

    // When: each completed pipeline is driven once more.
    let _ = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let _ = immediate.run(&mut immediate_pipeline, &mut immediate_context);
    let stepped_terminal = stepped.step(&mut stepped_pipeline, &mut stepped_context);
    let immediate_terminal = immediate.run(&mut immediate_pipeline, &mut immediate_context);

    // Then: terminal exhaustion is a typed executor error with no further action.
    assert_eq!(stepped_terminal, Err(RunError::Exhausted));
    assert_eq!(immediate_terminal, Err(RunError::Exhausted));
    assert_eq!(stepped_context.actions, ["S"]);
    assert_eq!(immediate_context.actions, ["I"]);
    println!("post_terminal stepped={stepped_terminal:?} immediate={immediate_terminal:?}");
    Ok(())
}
