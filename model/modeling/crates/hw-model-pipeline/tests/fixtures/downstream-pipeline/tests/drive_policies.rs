#![allow(missing_docs, reason = "external fixture test")]

use std::future::{Future, poll_fn};
use std::num::NonZeroUsize;
use std::pin::pin;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::task::{Context as TaskContext, Poll, Waker};

use hip::{
    AsyncExecutor, AsyncInteractionBackend, ImmediateExecutor, Operation, PipelineExt, Stage,
    StepExecutor, StepOutcome, StepResult,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Model {
    upstream_calls: usize,
    downstream_calls: usize,
    readiness: usize,
}

#[derive(Debug, Default)]
struct Source;

impl Stage<Model> for Source {
    type Input = u32;
    type Output = u32;
    type Error = &'static str;

    fn step(
        &mut self,
        model: &mut Model,
        input: &mut Option<Self::Input>,
    ) -> StepResult<u32, &'static str> {
        let Some(input) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        model.upstream_calls += 1;
        Ok(StepOutcome::Complete(Ok(input)))
    }
}

#[derive(Debug, Default)]
struct Sink {
    pending: Option<u32>,
}

impl Stage<Model> for Sink {
    type Input = u32;
    type Output = u32;
    type Error = &'static str;

    fn step(
        &mut self,
        model: &mut Model,
        input: &mut Option<Self::Input>,
    ) -> StepResult<u32, &'static str> {
        if self.pending.is_none() {
            self.pending = input.take();
        }
        if self.pending.is_none() {
            return Ok(StepOutcome::InProgress);
        }
        model.downstream_calls += 1;
        if model.readiness < 2 {
            return Ok(StepOutcome::InProgress);
        }
        Ok(StepOutcome::Complete(Ok(self
            .pending
            .take()
            .ok_or(hip::RunError::Exhausted)?
            + 22)))
    }
}

fn pipeline() -> impl Stage<Model, Input = u32, Output = u32, Error = &'static str> + Send {
    Source.then(Sink::default())
}

#[derive(Debug, Default)]
struct Backend(Mutex<Model>);

impl AsyncInteractionBackend<Model> for Backend {
    type Error = &'static str;

    async fn yield_now(&self) {
        let mut yielded = false;
        poll_fn(|context| {
            if yielded {
                Poll::Ready(())
            } else {
                yielded = true;
                context.waker().wake_by_ref();
                Poll::Pending
            }
        })
        .await;
    }

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepResult<O, E>, Self::Error>
    where
        F: FnOnce(&mut Model) -> StepResult<O, E> + Send,
    {
        Ok(interaction(&mut lock(&self.0)))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn block_on<F: Future>(future: F, backend: &Backend) -> Result<F::Output, &'static str> {
    let waker = Waker::noop();
    let mut context = TaskContext::from_waker(waker);
    let mut future = pin!(future);
    for _ in 0..32 {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return Ok(output);
        }
        let mut model = lock(&backend.0);
        model.readiness = model.downstream_calls.min(2);
    }
    Err("future exceeded fixture poll budget")
}

fn drive_immediate() -> Result<(u32, Model), Box<dyn std::error::Error>> {
    let mut stage = pipeline();
    let mut operation = Operation::new(20);
    let mut model = Model::default();
    let executor = ImmediateExecutor::new(NonZeroUsize::MIN);
    for readiness in [0, 0, 1, 2] {
        model.readiness = readiness;
        if let Ok(StepOutcome::Complete(Ok(output))) =
            executor.run(&mut stage, &mut operation, &mut model)
        {
            return Ok((output, model));
        }
    }
    Err("immediate pipeline did not complete".into())
}

fn drive_stepped() -> Result<(u32, Model), Box<dyn std::error::Error>> {
    let mut stage = pipeline();
    let mut operation = Operation::new(20);
    let mut model = Model::default();
    let executor = StepExecutor::new();
    for readiness in [0, 0, 1, 2] {
        model.readiness = readiness;
        if let Ok(StepOutcome::Complete(Ok(output))) =
            executor.step(&mut stage, &mut operation, &mut model)
        {
            return Ok((output, model));
        }
    }
    Err("stepped pipeline did not complete".into())
}

#[test]
fn same_static_pipeline_has_literal_policy_parity_without_replay()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: three instances of one statically composed source-then-sink pipeline.
    let immediate = drive_immediate()?;
    let stepped = drive_stepped()?;
    let executor = AsyncExecutor::new(Backend::default());
    let mut asynchronous = pipeline();

    // When: the async sink genuinely suspends twice before it becomes ready.
    let async_output = block_on(
        executor.run(&mut asynchronous, &mut Operation::new(20)),
        executor.backend(),
    )?
    .map_err(|error| std::io::Error::other(format!("async pipeline failed: {error:?}")))?;
    let async_model = lock(&executor.backend().0).clone();

    // Then: every policy yields 42 and no downstream pending replays the source.
    assert_eq!(
        immediate,
        (
            42,
            Model {
                upstream_calls: 1,
                downstream_calls: 3,
                readiness: 2
            }
        )
    );
    assert_eq!(stepped, immediate);
    assert_eq!((async_output, async_model), immediate);
    println!(
        "pipeline immediate=42 stepped=42 async=42 pending_resumes=2 upstream_calls=1 downstream_calls=3"
    );
    Ok(())
}
