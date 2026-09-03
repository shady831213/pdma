//! Static asynchronous executor contract tests.

use std::{
    future::{Future, pending},
    num::NonZeroUsize,
    pin::Pin,
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context as TaskContext, Poll, Waker},
};

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, AsyncRunError, ImmediateExecutor, PipelineExt,
    RunOutcome, Stage, StepExecutor, StepOutcome,
};

type Context = Vec<&'static str>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackendFault {
    Before,
    After,
    Wait,
}

struct ScriptedBackend {
    context: Mutex<Context>,
    before_open: AtomicBool,
    after_open: AtomicBool,
    interactions: AtomicUsize,
    generation: AtomicUsize,
    wait_checks: AtomicUsize,
    bump_after: AtomicBool,
    failure: Option<BackendFault>,
}

impl ScriptedBackend {
    const fn configured(before: bool, after: bool, failure: Option<BackendFault>) -> Self {
        Self {
            context: Mutex::new(Vec::new()),
            before_open: AtomicBool::new(before),
            after_open: AtomicBool::new(after),
            interactions: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
            wait_checks: AtomicUsize::new(0),
            bump_after: AtomicBool::new(false),
            failure,
        }
    }

    const fn failing(fault: BackendFault) -> Self {
        Self::configured(true, true, Some(fault))
    }
}

impl AsyncInteractionBackend<Context, usize> for ScriptedBackend {
    type Error = BackendFault;

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepOutcome<O, E, usize>, Self::Error>
    where
        F: FnOnce(&mut Context) -> StepOutcome<O, E, usize> + Send,
    {
        if self.failure == Some(BackendFault::Before) {
            return Err(BackendFault::Before);
        }
        if !self.before_open.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        let outcome = {
            self.interactions.fetch_add(1, Ordering::SeqCst);
            interaction(&mut lock(&self.context))
        };
        if self.bump_after.swap(false, Ordering::SeqCst) {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        if !self.after_open.load(Ordering::SeqCst) {
            pending::<()>().await;
        }
        match self.failure {
            Some(BackendFault::After) => Err(BackendFault::After),
            Some(BackendFault::Before | BackendFault::Wait) | None => Ok(outcome),
        }
    }

    async fn wait(&self, wait: usize) -> Result<(), Self::Error> {
        if self.failure == Some(BackendFault::Wait) {
            return Err(BackendFault::Wait);
        }
        self.wait_checks.fetch_add(1, Ordering::SeqCst);
        if self.generation.load(Ordering::SeqCst) == wait {
            pending::<()>().await;
        }
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Leaf {
    name: &'static str,
    result: Option<Result<&'static str, &'static str>>,
}

impl Leaf {
    const fn new(name: &'static str, result: Result<&'static str, &'static str>) -> Self {
        Self {
            name,
            result: Some(result),
        }
    }
}

impl Stage<Context> for Leaf {
    type Output = &'static str;
    type Error = &'static str;
    type Wait = usize;

    fn step(
        &mut self,
        context: &mut Context,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        self.result.take().map_or(StepOutcome::Exhausted, |result| {
            context.push(self.name);
            StepOutcome::Complete(result)
        })
    }
}

fn ready_chain()
-> impl Stage<Context, Output = &'static str, Error = &'static str, Wait = usize> + Send {
    Leaf::new("A", Ok("a"))
        .then(|_| Leaf::new("B", Ok("b")))
        .then(|_| Leaf::new("C", Ok("terminal")))
}

fn poll_once<T>(future: Pin<&mut impl Future<Output = T>>) -> Poll<T> {
    let waker = Waker::noop();
    let mut context = TaskContext::from_waker(waker);
    future.poll(&mut context)
}

fn assert_send(_: impl Future + Send) {}

#[tokio::test]
async fn parity_ready_and_error_match_synchronous_executors() {
    // Given: equivalent ready chains and a scripted async backend with open gates.
    let mut stepped_stage = ready_chain();
    let mut immediate_stage = ready_chain();
    let mut async_stage = ready_chain();
    let (mut stepped_context, mut immediate_context) = (Vec::new(), Vec::new());
    let immediate = ImmediateExecutor::new(NonZeroUsize::new(3).unwrap_or(NonZeroUsize::MIN));
    let executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));

    // When: each executor drives its equivalent pipeline to completion.
    let _ = StepExecutor::new().step(&mut stepped_stage, &mut stepped_context);
    let _ = StepExecutor::new().step(&mut stepped_stage, &mut stepped_context);
    let stepped_result = StepExecutor::new().step(&mut stepped_stage, &mut stepped_context);
    let immediate_result = immediate.run(&mut immediate_stage, &mut immediate_context);
    let async_result = executor.run(&mut async_stage).await;

    // Then: output, action order, and one async closure per leaf are identical.
    assert_eq!(stepped_result, Ok(RunOutcome::Complete(Ok("terminal"))));
    assert_eq!(immediate_result, stepped_result);
    assert_eq!(async_result, Ok("terminal"));
    assert_eq!(stepped_context, ["A", "B", "C"]);
    assert_eq!(immediate_context, stepped_context);
    assert_eq!(*lock(&executor.backend().context), stepped_context);
    assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 3);

    let error_executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
    let mut error_stage = Leaf::new("E", Err("stage"));
    let async_error = error_executor.run(&mut error_stage).await;
    error_stage.result = Some(Err("stage"));
    let sync_error = immediate.run(&mut error_stage, &mut immediate_context);
    assert_eq!(async_error, Err(AsyncRunError::Stage("stage")));
    assert_eq!(sync_error, Ok(RunOutcome::Complete(Err("stage"))));
    let send_executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
    let mut send_stage = Leaf::new("send", Ok("done"));
    assert_send(send_executor.run(&mut send_stage));
}

#[tokio::test]
async fn lost_wakeup_rechecks_generation_before_registering() {
    // Given: a pending stage whose interaction publishes generation one before wait starts.
    let executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
    executor.backend().bump_after.store(true, Ordering::SeqCst);
    let mut stage = PendingOnce(false);
    let mut stepped_stage = PendingOnce(false);

    // When: the async executor receives generation zero after readiness already changed.
    let result = executor.run(&mut stage).await;
    let pending = StepExecutor::new().step(&mut stepped_stage, &mut Vec::new());
    let resumed = StepExecutor::new().step(&mut stepped_stage, &mut Vec::new());

    // Then: pending ownership matches stepping and the stale generation completes on one recheck.
    assert_eq!(result, Ok("after-wake"));
    assert_eq!(pending, Ok(RunOutcome::Pending(0)));
    assert_eq!(resumed, Ok(RunOutcome::Complete(Ok("after-wake"))));
    assert_eq!(executor.backend().generation.load(Ordering::SeqCst), 1);
    assert_eq!(executor.backend().wait_checks.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_before_and_after_commit_preserves_exact_state() {
    // Given: a backend deterministically gated before context access.
    let pre_executor = AsyncExecutor::new(ScriptedBackend::configured(false, true, None));
    let mut pre_stage = Leaf::new("never", Ok("done"));

    // When: one pending poll is dropped before the gate opens.
    let mut run = Box::pin(pre_executor.run(&mut pre_stage));
    assert!(poll_once(run.as_mut()).is_pending());
    drop(run);

    // Then: neither the backend closure nor the stage interaction ran.
    assert_eq!(
        pre_executor.backend().interactions.load(Ordering::SeqCst),
        0
    );
    assert!(lock(&pre_executor.backend().context).is_empty());

    // Given: a two-leaf pipeline gated after its first closure commits.
    let executor = AsyncExecutor::new(ScriptedBackend::configured(true, false, None));
    let mut stage =
        Leaf::new("upstream", Ok("input")).then(|_| Leaf::new("downstream", Ok("done")));

    // When: the run is dropped after the upstream closure, then resumed with the gate open.
    let mut run = Box::pin(executor.run(&mut stage));
    assert!(poll_once(run.as_mut()).is_pending());
    drop(run);
    assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 1);
    executor.backend().after_open.store(true, Ordering::SeqCst);
    let resumed = executor.run(&mut stage).await;

    // Then: the committed upstream action occurs once and downstream completes once.
    assert_eq!(resumed, Ok("done"));
    assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 2);
    assert_eq!(
        *lock(&executor.backend().context),
        ["upstream", "downstream"]
    );
}

#[tokio::test]
async fn adverse_errors_preserve_backend_stage_and_exhausted_variants() {
    // Given: separate backends failing around access, during wait, plus terminal stages.
    let before = AsyncExecutor::new(ScriptedBackend::failing(BackendFault::Before));
    let after = AsyncExecutor::new(ScriptedBackend::failing(BackendFault::After));
    let waiting = AsyncExecutor::new(ScriptedBackend::failing(BackendFault::Wait));
    let mut before_stage = Leaf::new("before", Ok("done"));
    let mut after_stage = Leaf::new("after", Ok("done"));
    let mut wait_stage = PendingOnce(false);
    let mut exhausted_stage = Leaf::new("terminal", Ok("done"));
    let complete = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));

    // When: each adverse boundary is driven exactly once.
    let before_result = before.run(&mut before_stage).await;
    let after_result = after.run(&mut after_stage).await;
    let wait_result = waiting.run(&mut wait_stage).await;
    let _ = complete.run(&mut exhausted_stage).await;
    let exhausted_result = complete.run(&mut exhausted_stage).await;

    // Then: every boundary has a distinct typed, literal result.
    assert_eq!(
        before_result,
        Err(AsyncRunError::BackendInteraction(BackendFault::Before))
    );
    assert_eq!(
        after_result,
        Err(AsyncRunError::BackendInteraction(BackendFault::After))
    );
    assert_eq!(
        wait_result,
        Err(AsyncRunError::BackendWait(BackendFault::Wait))
    );
    assert_eq!(exhausted_result, Err(AsyncRunError::Exhausted));
    assert_eq!(before.backend().interactions.load(Ordering::SeqCst), 0);
    assert_eq!(after.backend().interactions.load(Ordering::SeqCst), 1);
}

struct PendingOnce(bool);

impl Stage<Context> for PendingOnce {
    type Output = &'static str;
    type Error = &'static str;
    type Wait = usize;

    fn step(&mut self, _: &mut Context) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        if self.0 {
            StepOutcome::Complete(Ok("after-wake"))
        } else {
            self.0 = true;
            StepOutcome::Pending(0)
        }
    }
}
