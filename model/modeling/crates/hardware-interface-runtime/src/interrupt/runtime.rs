use super::{
    HandlerRegistry, InterruptDispatchError, InterruptDispatcher, InterruptEnvelope,
    InterruptHandler, InterruptRegistration, InterruptSource, InterruptSummary,
    NoopInterruptObserver, dispatcher::run_dispatch,
};
use crate::SoftwareCpu;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Runs software and its interrupt handlers on one CPU using the canonical dispatcher.
#[derive(Debug)]
pub struct InterruptRuntime<S> {
    cpu: Arc<SoftwareCpu>,
    source: S,
}

impl<S: InterruptSource> InterruptRuntime<S> {
    /// Connects an externally supplied interrupt source to a software CPU.
    pub const fn new(cpu: Arc<SoftwareCpu>, source: S) -> Self {
        Self { cpu, source }
    }

    /// Runs a complete program with ISR registration and scoped interrupt dispatch.
    ///
    /// Program return stops dispatch after any accepted handler finishes. A dispatcher
    /// failure cancels the program and is returned. Dropping this future cancels both;
    /// no background tasks survive it. The registration capability cannot escape.
    ///
    /// ```compile_fail
    /// use hardware_interface_runtime::{InterruptRuntime, InterruptSource, InterruptHandler};
    /// async fn escape<S, H>(runtime: InterruptRuntime<S>)
    /// where S: InterruptSource, H: InterruptHandler<S::Notification> {
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
    {
        let registry = Mutex::new(HandlerRegistry::new());
        let registration = InterruptRegistration {
            cpu: &self.cpu,
            registry: &registry,
        };
        let (shutdown, receiver) = InterruptDispatcher::shutdown_channel();
        let dispatch = run_dispatch(
            Arc::clone(&self.cpu),
            &registry,
            self.source,
            Arc::new(NoopInterruptObserver),
            receiver.shutdown,
        );
        let main = async {
            let output = self.cpu.run_main(program(registration)).await;
            shutdown.request_shutdown();
            Ok(output)
        };
        tokio::try_join!(main, dispatch)
    }
}
