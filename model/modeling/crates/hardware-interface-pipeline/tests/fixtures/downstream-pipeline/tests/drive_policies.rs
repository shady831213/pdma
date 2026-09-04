#![allow(missing_docs, reason = "external fixture test")]

use std::future::{Future, poll_fn};
use std::num::NonZeroUsize;
use std::pin::pin;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::task::{Context as TaskContext, Poll, Waker};

use hip::{
    AsyncExecutor, AsyncInteractionBackend, ImmediateExecutor, PipelineExt, RunOutcome, Stage,
    StepExecutor, StepOutcome,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Model {
    upstream_calls: usize,
    downstream_calls: usize,
    readiness: usize,
}

#[derive(Debug, Default)]
struct Source(bool);

impl Stage<Model> for Source {
    type Output = u32;
    type Error = &'static str;
    type Wait = usize;

    fn step(&mut self, model: &mut Model) -> StepOutcome<u32, &'static str, usize> {
        if self.0 {
            return StepOutcome::Exhausted;
        }
        self.0 = true;
        model.upstream_calls += 1;
        StepOutcome::Complete(Ok(20))
    }
}

#[derive(Debug)]
struct Sink {
    input: u32,
    done: bool,
}

impl Stage<Model> for Sink {
    type Output = u32;
    type Error = &'static str;
    type Wait = usize;

    fn step(&mut self, model: &mut Model) -> StepOutcome<u32, &'static str, usize> {
        if self.done {
            return StepOutcome::Exhausted;
        }
        model.downstream_calls += 1;
        if model.readiness < 2 {
            return StepOutcome::Pending(model.readiness);
        }
        self.done = true;
        StepOutcome::Complete(Ok(self.input + 22))
    }
}

fn pipeline() -> impl Stage<Model, Output = u32, Error = &'static str, Wait = usize> + Send {
    Source::default().then(|input| Sink { input, done: false })
}

#[derive(Debug, Default)]
struct Backend(Mutex<Model>);

impl AsyncInteractionBackend<Model, usize> for Backend {
    type Error = &'static str;

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepOutcome<O, E, usize>, Self::Error>
    where
        F: FnOnce(&mut Model) -> StepOutcome<O, E, usize> + Send,
    {
        Ok(interaction(&mut lock(&self.0)))
    }

    async fn wait(&self, wait: usize) -> Result<(), Self::Error> {
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
        lock(&self.0).readiness = wait + 1;
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn block_on<F: Future>(future: F) -> Result<F::Output, &'static str> {
    let waker = Waker::noop();
    let mut context = TaskContext::from_waker(waker);
    let mut future = pin!(future);
    for _ in 0..32 {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return Ok(output);
        }
    }
    Err("future exceeded fixture poll budget")
}

fn drive_immediate() -> Result<(u32, Model), Box<dyn std::error::Error>> {
    let mut stage = pipeline();
    let mut model = Model::default();
    let executor = ImmediateExecutor::new(NonZeroUsize::new(8).ok_or("nonzero budget")?);
    for readiness in 0..=2 {
        model.readiness = readiness;
        if let Ok(RunOutcome::Complete(Ok(output))) = executor.run(&mut stage, &mut model) {
            return Ok((output, model));
        }
    }
    Err("immediate pipeline did not complete".into())
}

fn drive_stepped() -> Result<(u32, Model), Box<dyn std::error::Error>> {
    let mut stage = pipeline();
    let mut model = Model::default();
    let executor = StepExecutor::new();
    for readiness in [0, 0, 1, 2] {
        model.readiness = readiness;
        if let Ok(RunOutcome::Complete(Ok(output))) = executor.step(&mut stage, &mut model) {
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
    let async_output = block_on(executor.run(&mut asynchronous))?
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
