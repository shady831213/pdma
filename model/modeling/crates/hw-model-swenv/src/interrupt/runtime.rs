use super::{
    HandlerRegistry, InterruptDispatchError, InterruptDispatcher, InterruptRegistration,
    InterruptSummary, NoopInterruptObserver, dispatcher::run_dispatch,
};
use crate::SoftwareEnv;
use hw_model_irq::{InterruptEnvelope, InterruptHandler, InterruptSource};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Runs software and its interrupt handlers on one environment using the canonical dispatcher.
///
/// This is scoped program/ISR orchestration over Tokio, not a general async
/// executor. Start with [`Self::run_main`]; use [`InterruptDispatcher`] only when
/// assembly must own a separately spawned dispatch task.
#[derive(Debug)]
pub struct InterruptRuntime<S> {
    env: Arc<SoftwareEnv>,
    source: S,
}

impl<S: InterruptSource> InterruptRuntime<S> {
    /// Connects an externally supplied interrupt source to a software environment.
    pub const fn new(env: Arc<SoftwareEnv>, source: S) -> Self {
        Self { env, source }
    }

    /// Runs a complete program with ISR registration and scoped interrupt dispatch.
    ///
    /// Program return stops dispatch after any accepted handler finishes. A dispatcher
    /// failure cancels the program and is returned. Dropping this future cancels both;
    /// no background tasks survive it. The registration capability cannot escape.
    ///
    /// # Errors
    /// Returns [`InterruptDispatchError`] for source or handler failure, or an
    /// unregistered vector. The program's own result is returned as `T`,
    /// not flattened into this error: if `T` is a `Result`, inspect both layers.
    ///
    /// # Scheduling
    /// Install handlers before their vectors can arrive. A handler may await
    /// external work, but must not wait for this program to advance: an accepted
    /// handler retains the environment turn until it finishes.
    ///
    /// ```compile_fail
    /// use hw_model_swenv::{InterruptRuntime};
    /// use hw_model_irq::{InterruptEnvelope, InterruptSource, InterruptHandler};
    /// async fn escape<S, H>(runtime: InterruptRuntime<S>)
    /// where S: InterruptSource, H: InterruptHandler<S::Notification>,
    ///       <S::Notification as InterruptEnvelope>::Vector: Ord {
    ///     let _escaped = runtime.run_main::<H, _>(async |registration| registration).await;
    /// }
    /// ```
    pub async fn run_main<H, T>(
        self,
        program: impl for<'a> AsyncFnOnce(InterruptRegistration<'a, S::Notification, H>) -> T,
    ) -> Result<
        (T, InterruptSummary),
        InterruptDispatchError<<S::Notification as InterruptEnvelope>::Vector, S::Error, H::Error>,
    >
    where
        H: InterruptHandler<S::Notification>,
        <S::Notification as InterruptEnvelope>::Vector: Ord,
    {
        let registry = Mutex::new(HandlerRegistry::new());
        let registration = InterruptRegistration {
            env: &self.env,
            registry: &registry,
        };
        let (shutdown, receiver) = InterruptDispatcher::shutdown_channel();
        let dispatch = run_dispatch(
            Arc::clone(&self.env),
            &registry,
            self.source,
            Arc::new(NoopInterruptObserver),
            receiver.shutdown,
        );
        let main = async {
            let output = self.env.run_main(program(registration)).await;
            shutdown.request_shutdown();
            Ok(output)
        };
        tokio::try_join!(main, dispatch)
    }
}
