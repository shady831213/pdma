//! Transitional host IRQ API forwarded to the canonical runtime dispatcher.

use crate::{AsyncDevicePort, ModelError, ModelResult, SoftwareCpu};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptDispatcher, InterruptShutdown, InterruptShutdownReceiver,
};
use pdma_model_contract::{IrqNotification, IrqVector, TraceEvent};
use std::{
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
};

mod bootstrap;
mod compat;
mod trace;

pub use bootstrap::{IrqDeliveryBootstrap, IrqDeliveryRunner};
use compat::{CompatHandler, CompatNotification, CompatObserver, CompatSource};
use trace::HostIrqTrace;

/// Boxed asynchronous PDMA handler turn.
pub type IrqHandlerFuture<'a> = Pin<Box<dyn Future<Output = ModelResult<()>> + Send + 'a>>;

/// Transitional handler signature retained through Task 26.
pub trait IrqHandler: Send {
    /// Handles one notification in the runtime-owned CPU turn.
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_>;
}

/// Transitional stateful handler signature retained through Task 26.
pub trait StatefulIrqHandler: Send {
    /// Handles one notification while borrowing persistent state.
    fn handle(
        &mut self,
        notification: IrqNotification,
    ) -> impl Future<Output = ModelResult<()>> + Send;
}

struct FunctionIrqHandler<F>(F);
struct StatefulIrqHandlerAdapter<H>(H);

/// Adapts an async function into the transitional handler signature.
pub const fn irq_handler<F, Fut>(handler: F) -> impl IrqHandler
where
    F: FnMut(IrqNotification) -> Fut + Send,
    Fut: Future<Output = ModelResult<()>> + Send + 'static,
{
    FunctionIrqHandler(handler)
}

/// Adapts persistent async state into the transitional handler signature.
pub const fn stateful_irq_handler<H>(handler: H) -> impl IrqHandler
where
    H: StatefulIrqHandler,
{
    StatefulIrqHandlerAdapter(handler)
}

impl<F, Fut> IrqHandler for FunctionIrqHandler<F>
where
    F: FnMut(IrqNotification) -> Fut + Send,
    Fut: Future<Output = ModelResult<()>> + Send + 'static,
{
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin((self.0)(notification))
    }
}

impl<H: StatefulIrqHandler> IrqHandler for StatefulIrqHandlerAdapter<H> {
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(StatefulIrqHandler::handle(&mut self.0, notification))
    }
}

type RuntimeRegistry<'handler> =
    HandlerRegistry<IrqVector, CompatNotification, CompatHandler<'handler>>;

/// Type-state marker for a delivery adapter without a registered handler.
#[derive(Debug)]
pub struct NoHandler(PhantomData<()>);

/// Transitional registry forwarding duplicate and freeze semantics to the runtime.
pub struct IrqHandlerRegistry<'handler> {
    inner: RuntimeRegistry<'handler>,
}

impl fmt::Debug for IrqHandlerRegistry<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(formatter)
    }
}

impl<'handler> IrqHandlerRegistry<'handler> {
    const fn new() -> Self {
        Self {
            inner: HandlerRegistry::new(),
        }
    }

    /// Adds one vector handler without replacing the runtime registration.
    pub fn register<H>(&mut self, vector: IrqVector, handler: H) -> ModelResult<()>
    where
        H: IrqHandler + 'handler,
    {
        self.inner
            .register(vector, CompatHandler::new(handler))
            .map_err(|duplicate| ModelError::DuplicateIrqVector(*duplicate.vector()))
    }

    fn freeze(self) -> ModelResult<HasHandlers<'handler>> {
        if self.inner.is_empty() {
            Err(ModelError::EmptyIrqHandlerRegistry)
        } else {
            Ok(HasHandlers {
                registry: self.inner,
            })
        }
    }
}

/// Type-state marker carrying the runtime's frozen registry.
pub struct HasHandlers<'handler> {
    registry: RuntimeRegistry<'handler>,
}

impl fmt::Debug for HasHandlers<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.registry.fmt(formatter)
    }
}

/// Transitional builder whose execution is the canonical runtime dispatcher.
#[derive(Debug)]
pub struct IrqDelivery<P, H> {
    port: P,
    cpu: SoftwareCpu,
    handler: H,
    shutdown: InterruptShutdown,
    receiver: InterruptShutdownReceiver,
    trace: Arc<Mutex<HostIrqTrace>>,
}

/// Signal-only compatibility handle for the canonical runtime dispatcher.
#[derive(Debug, Clone)]
pub struct IrqDeliveryHandle {
    shutdown: InterruptShutdown,
    trace: Arc<Mutex<HostIrqTrace>>,
}

/// Final successful-delivery count projected from the runtime summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqDeliverySummary {
    deliveries: usize,
}

impl<P> IrqDelivery<P, NoHandler> {
    /// Creates an unregistered compatibility builder.
    pub fn new(port: P, cpu: SoftwareCpu) -> Self {
        let (shutdown, receiver) = InterruptDispatcher::shutdown_channel();
        Self {
            port,
            cpu,
            handler: NoHandler(PhantomData),
            shutdown,
            receiver,
            trace: Arc::new(Mutex::new(HostIrqTrace::default())),
        }
    }

    /// Registers the first handler in the runtime registry.
    pub fn register_handler<'handler, H>(
        self,
        vector: IrqVector,
        handler: H,
    ) -> (IrqDelivery<P, HasHandlers<'handler>>, IrqDeliveryHandle)
    where
        H: IrqHandler + 'handler,
    {
        let handlers = HasHandlers {
            registry: HandlerRegistry::with_handler(vector, CompatHandler::new(handler)),
        };
        self.with_handlers(handlers)
    }

    fn with_handlers(
        self,
        handler: HasHandlers<'_>,
    ) -> (IrqDelivery<P, HasHandlers<'_>>, IrqDeliveryHandle) {
        let handle = IrqDeliveryHandle {
            shutdown: self.shutdown.clone(),
            trace: Arc::clone(&self.trace),
        };
        (
            IrqDelivery {
                port: self.port,
                cpu: self.cpu,
                handler,
                shutdown: self.shutdown,
                receiver: self.receiver,
                trace: self.trace,
            },
            handle,
        )
    }
}

impl<'handler, P: AsyncDevicePort + 'static> IrqDelivery<P, HasHandlers<'handler>> {
    /// Adds a handler before runtime execution.
    pub fn register_handler<H>(&mut self, vector: IrqVector, handler: H) -> ModelResult<()>
    where
        H: IrqHandler + 'handler,
    {
        self.handler
            .registry
            .register(vector, CompatHandler::new(handler))
            .map_err(|duplicate| ModelError::DuplicateIrqVector(*duplicate.vector()))
    }

    /// Runs the single canonical runtime dispatcher in this caller-owned future.
    pub async fn run(self) -> ModelResult<IrqDeliverySummary> {
        let observer = Arc::new(CompatObserver::new(Arc::clone(&self.trace)));
        let registry = self
            .handler
            .registry
            .freeze()
            .map_err(|_| ModelError::EmptyIrqHandlerRegistry)?;
        let _shutdown = self.shutdown;
        let result = InterruptDispatcher::run(
            Arc::new(self.cpu.runtime()),
            registry,
            CompatSource::new(self.port),
            observer,
            self.receiver,
        )
        .await;
        compat::project_result(&result, &self.trace)
    }
}

impl IrqDeliveryHandle {
    /// Requests cooperative runtime shutdown.
    pub fn request_shutdown(&self) {
        self.shutdown.request_shutdown();
    }

    /// Returns runtime-observed lifecycle events.
    pub async fn trace_events(&self) -> Vec<TraceEvent> {
        let events = self
            .trace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events()
            .to_vec();
        std::future::ready(events).await
    }
}

impl IrqDeliverySummary {
    /// Number of successful runtime handler turns.
    pub const fn deliveries(self) -> usize {
        self.deliveries
    }
}
