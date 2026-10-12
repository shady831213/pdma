//! Static asynchronous executor contract tests.

mod async_executor {
    mod backend;
    mod repeat;

    use std::{
        future::Future,
        num::NonZeroUsize,
        pin::Pin,
        sync::atomic::Ordering,
        task::{Context as TaskContext, Poll, Waker},
    };

    use hardware_interface_pipeline::{
        AsyncExecutor, AsyncRunError, ImmediateExecutor, Operation, PipelineExt, RunError, Stage,
        StepExecutor, StepOutcome, StepResult, Then,
    };

    type Context = Vec<&'static str>;

    use backend::{BackendFault, ScriptedBackend, lock};

    struct Leaf {
        name: &'static str,
        result: Result<&'static str, &'static str>,
    }

    impl Leaf {
        const fn new(name: &'static str, result: Result<&'static str, &'static str>) -> Self {
            Self { name, result }
        }
    }

    impl Stage<Context> for Leaf {
        type Input = &'static str;
        type Output = &'static str;
        type Error = &'static str;
        type State = ();

        fn start(_: Self::Input) -> Self::State {}

        fn step(
            &mut self,
            context: &mut Context,
            (): &mut Self::State,
        ) -> StepResult<Self::Output, Self::Error> {
            context.push(self.name);
            Ok(StepOutcome::Complete(self.result))
        }
    }

    fn ready_chain() -> Then<Then<Leaf, Leaf>, Leaf> {
        Leaf::new("A", Ok("a"))
            .then(Leaf::new("B", Ok("b")))
            .then(Leaf::new("C", Ok("terminal")))
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
        let mut stepped_hw = ready_chain();
        let mut immediate_hw = ready_chain();
        let mut async_stage = ready_chain();
        let mut stepped_op = Operation::new("request");
        let mut immediate_op = Operation::new("request");
        let mut async_op = Operation::new("request");
        let (mut stepped_cx, mut immediate_cx) = (Vec::new(), Vec::new());
        let immediate = ImmediateExecutor::new(NonZeroUsize::new(3).unwrap_or(NonZeroUsize::MIN));
        let executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));

        // When: each executor drives its equivalent pipeline to completion.
        let step = StepExecutor::new();
        let _ = step.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
        let _ = step.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
        let stepped_result = step.step(&mut stepped_hw, &mut stepped_op, &mut stepped_cx);
        let immediate_result =
            immediate.run(&mut immediate_hw, &mut immediate_op, &mut immediate_cx);
        let async_result = executor.run(&mut async_stage, &mut async_op).await;

        // Then: output, action order, and one async closure per leaf are identical.
        assert_eq!(stepped_result, Ok(StepOutcome::Complete(Ok("terminal"))));
        assert_eq!(immediate_result, stepped_result);
        assert_eq!(async_result, Ok("terminal"));
        assert_eq!(stepped_cx, ["A", "B", "C"]);
        assert_eq!(immediate_cx, stepped_cx);
        assert_eq!(*lock(&executor.backend().context), stepped_cx);
        assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 3);

        let error_executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
        let mut error_stage = Leaf::new("E", Err("stage"));
        let async_error = error_executor
            .run(&mut error_stage, &mut Operation::new("request"))
            .await;
        let sync_error = immediate.run(
            &mut error_stage,
            &mut Operation::new("request"),
            &mut immediate_cx,
        );
        assert_eq!(async_error, Err(AsyncRunError::Stage("stage")));
        assert_eq!(sync_error, Ok(StepOutcome::Complete(Err("stage"))));
        let send_executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
        let mut send_stage = Leaf::new("send", Ok("done"));
        assert_send(send_executor.run(&mut send_stage, &mut Operation::new("request")));
    }

    #[tokio::test]
    async fn unfinished_stage_is_revisited_without_a_notification() {
        // Given: a stage retaining unfinished work and an immediately ready backend.
        let executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
        let mut stage = TwoStep;
        let mut stepped_hw = TwoStep;
        let mut stepped_op = Operation::new(());

        // When: the async executor cooperatively revisits the retained work.
        let result = executor.run(&mut stage, &mut Operation::new(())).await;
        let pending = StepExecutor::new().step(&mut stepped_hw, &mut stepped_op, &mut Vec::new());
        let resumed = StepExecutor::new().step(&mut stepped_hw, &mut stepped_op, &mut Vec::new());

        // Then: both drivers preserve the result without a separate wake protocol.
        assert_eq!(result, Ok("after-wake"));
        assert_eq!(pending, Ok(StepOutcome::InProgress));
        assert_eq!(resumed, Ok(StepOutcome::Complete(Ok("after-wake"))));
        assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cancellation_before_and_after_commit_preserves_exact_state() {
        // Given: a backend deterministically gated before context access.
        let pre_executor = AsyncExecutor::new(ScriptedBackend::configured(false, true, None));
        let mut pre_stage = Leaf::new("never", Ok("done"));
        let mut pre_op = Operation::new("request");

        // When: one pending poll is dropped before the gate opens.
        let mut run = Box::pin(pre_executor.run(&mut pre_stage, &mut pre_op));
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
            Leaf::new("upstream", Ok("input")).then(Leaf::new("downstream", Ok("done")));
        let mut operation = Operation::new("request");

        // When: the run is dropped after the upstream closure, then resumed with the gate open.
        let mut run = Box::pin(executor.run(&mut stage, &mut operation));
        assert!(poll_once(run.as_mut()).is_pending());
        drop(run);
        assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 1);
        executor.backend().after_open.store(true, Ordering::SeqCst);
        let resumed = executor.run(&mut stage, &mut operation).await;

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
        // Given: separate backends failing around access, plus terminal stages.
        let before = AsyncExecutor::new(ScriptedBackend::failing(BackendFault::Before));
        let after = AsyncExecutor::new(ScriptedBackend::failing(BackendFault::After));
        let mut before_stage = Leaf::new("before", Ok("done"));
        let mut after_stage = Leaf::new("after", Ok("done"));
        let mut exhausted_stage = Leaf::new("terminal", Ok("done"));
        let complete = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
        let mut terminal_op = Operation::new("request");

        // When: each adverse boundary is driven exactly once.
        let before_result = before
            .run(&mut before_stage, &mut Operation::new("request"))
            .await;
        let after_result = after
            .run(&mut after_stage, &mut Operation::new("request"))
            .await;
        let _ = complete.run(&mut exhausted_stage, &mut terminal_op).await;
        let exhausted_result = complete.run(&mut exhausted_stage, &mut terminal_op).await;

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
            exhausted_result,
            Err(AsyncRunError::Run(RunError::Exhausted))
        );
        assert_eq!(before.backend().interactions.load(Ordering::SeqCst), 0);
        assert_eq!(after.backend().interactions.load(Ordering::SeqCst), 1);
    }

    struct TwoStep;

    impl Stage<Context> for TwoStep {
        type Input = ();
        type Output = &'static str;
        type Error = &'static str;
        type State = bool;

        fn start((): Self::Input) -> Self::State {
            false
        }

        fn step(
            &mut self,
            _: &mut Context,
            state: &mut Self::State,
        ) -> StepResult<Self::Output, Self::Error> {
            if *state {
                Ok(StepOutcome::Complete(Ok("after-wake")))
            } else {
                *state = true;
                Ok(StepOutcome::InProgress)
            }
        }
    }
}
