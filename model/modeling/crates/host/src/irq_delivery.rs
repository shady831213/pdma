//! Host-owned IRQ delivery actor.

use crate::ModelError;
use crate::ModelResult;

mod bootstrap;
mod trace;

use crate::{AsyncDevicePort, SoftwareCpu};
pub use bootstrap::{IrqDeliveryBootstrap, IrqDeliveryRunner};
use pdma_model_contract::{IrqNotification, IrqVector, TraceEvent};
use std::{
    collections::{BTreeMap, btree_map::Entry},
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
};
use tokio::sync::watch;
use trace::HostIrqTrace;

/// Boxed asynchronous PDMA handler turn.
pub type IrqHandlerFuture<'a> = Pin<Box<dyn Future<Output = ModelResult<()>> + Send + 'a>>;

/// Host handler invoked once for every delivered IRQ notification.
pub trait IrqHandler: Send {
    /// Handles one notification while holding the modeled CPU token.
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_>;
}

/// Persistent handler state with an awaitable, statically dispatched handler turn.
pub trait StatefulIrqHandler: Send {
    /// Handles one notification while borrowing the persistent handler state.
    fn handle(
        &mut self,
        notification: IrqNotification,
    ) -> impl Future<Output = ModelResult<()>> + Send;
}

struct FunctionIrqHandler<F>(F);

struct StatefulIrqHandlerAdapter<H>(H);

/// Adapts an async function or closure into a type-erased IRQ handler.
pub const fn irq_handler<F, Fut>(handler: F) -> impl IrqHandler
where
    F: FnMut(IrqNotification) -> Fut + Send,
    Fut: Future<Output = ModelResult<()>> + Send + 'static,
{
    FunctionIrqHandler(handler)
}

/// Adapts persistent async handler state into a type-erased IRQ handler.
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

impl<H> IrqHandler for StatefulIrqHandlerAdapter<H>
where
    H: StatefulIrqHandler,
{
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(StatefulIrqHandler::handle(&mut self.0, notification))
    }
}

/// Type-state marker for a delivery actor without a registered handler.
#[derive(Debug)]
pub struct NoHandler(PhantomData<()>);

/// Mutable software registration surface for a fixed host IRQ handler registry.
pub struct IrqHandlerRegistry<'handler> {
    handlers: BTreeMap<IrqVector, Box<dyn IrqHandler + 'handler>>,
}

impl fmt::Debug for IrqHandlerRegistry<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IrqHandlerRegistry")
            .field("vectors", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl<'handler> IrqHandlerRegistry<'handler> {
    const fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }

    /// Adds one vector handler without replacing an existing registration.
    pub fn register<H>(&mut self, vector: IrqVector, handler: H) -> ModelResult<()>
    where
        H: IrqHandler + 'handler,
    {
        match self.handlers.entry(vector) {
            Entry::Vacant(entry) => {
                entry.insert(Box::new(handler));
                Ok(())
            }
            Entry::Occupied(_) => Err(ModelError::DuplicateIrqVector(vector)),
        }
    }

    fn into_handler_state(self) -> ModelResult<HasHandlers<'handler>> {
        if self.handlers.is_empty() {
            return Err(ModelError::EmptyIrqHandlerRegistry);
        }
        Ok(HasHandlers { registry: self })
    }
}

/// Type-state marker carrying a non-empty vector-to-handler registry.
pub struct HasHandlers<'handler> {
    registry: IrqHandlerRegistry<'handler>,
}

impl fmt::Debug for HasHandlers<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HasHandlers")
            .field("registry", &self.registry)
            .finish()
    }
}

/// Host-owned actor adapting device IRQ notifications into handler turns.
#[derive(Debug)]
pub struct IrqDelivery<P, H> {
    port: P,
    cpu: SoftwareCpu,
    handler: H,
    shutdown_sender: watch::Sender<bool>,
    shutdown: watch::Receiver<bool>,
    trace: Arc<tokio::sync::Mutex<HostIrqTrace>>,
}

/// Host control surface for one IRQ delivery actor.
#[derive(Debug, Clone)]
pub struct IrqDeliveryHandle {
    shutdown: watch::Sender<bool>,
    trace: Arc<tokio::sync::Mutex<HostIrqTrace>>,
}

/// Final deterministic accounting from one IRQ delivery run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqDeliverySummary {
    deliveries: usize,
}

impl<P> IrqDelivery<P, NoHandler> {
    /// Creates a delivery actor builder with no software handler registered yet.
    pub fn new(port: P, cpu: SoftwareCpu) -> Self {
        let (shutdown_sender, shutdown) = watch::channel(false);
        let trace = Arc::new(tokio::sync::Mutex::new(HostIrqTrace::default()));
        Self {
            port,
            cpu,
            handler: NoHandler(PhantomData),
            shutdown_sender,
            shutdown,
            trace,
        }
    }

    /// Registers the first vector handler and makes the actor runnable.
    pub fn register_handler<'handler, H>(
        self,
        vector: IrqVector,
        handler: H,
    ) -> (IrqDelivery<P, HasHandlers<'handler>>, IrqDeliveryHandle)
    where
        H: IrqHandler + 'handler,
    {
        let mut handlers: BTreeMap<IrqVector, Box<dyn IrqHandler + 'handler>> = BTreeMap::new();
        handlers.insert(vector, Box::new(handler));
        self.with_handlers(HasHandlers {
            registry: IrqHandlerRegistry { handlers },
        })
    }

    fn with_handlers(
        self,
        handler: HasHandlers<'_>,
    ) -> (IrqDelivery<P, HasHandlers<'_>>, IrqDeliveryHandle) {
        let handle = IrqDeliveryHandle {
            shutdown: self.shutdown_sender.clone(),
            trace: Arc::clone(&self.trace),
        };
        (
            IrqDelivery {
                port: self.port,
                cpu: self.cpu,
                handler,
                shutdown_sender: self.shutdown_sender,
                shutdown: self.shutdown,
                trace: self.trace,
            },
            handle,
        )
    }
}

impl<'handler, P> IrqDelivery<P, HasHandlers<'handler>>
where
    P: AsyncDevicePort,
{
    /// Adds a handler before actor execution without replacing existing vectors.
    pub fn register_handler<H>(&mut self, vector: IrqVector, handler: H) -> ModelResult<()>
    where
        H: IrqHandler + 'handler,
    {
        self.handler.registry.register(vector, handler)
    }

    /// Runs until host shutdown is requested or the device notification stream closes.
    pub async fn run(mut self) -> ModelResult<IrqDeliverySummary> {
        let mut deliveries = 0usize;
        loop {
            let notification = tokio::select! {
                biased;
                changed = self.shutdown.changed() => {
                    if changed.is_err() || *self.shutdown.borrow() {
                        break;
                    }
                    continue;
                }
                notification = self.port.wait_irq_notification() => notification,
            };
            let Some(notification) = notification else {
                break;
            };
            let vector = notification.vector();
            let handler = self
                .handler
                .registry
                .handlers
                .get_mut(&vector)
                .ok_or(ModelError::UnregisteredIrqVector(vector))?;
            let trace = Arc::clone(&self.trace);
            let result = self
                .cpu
                .run_irq(async move {
                    trace.lock().await.handler_started(notification);
                    let result = handler.handle(notification).await;
                    trace.lock().await.handler_completed(result.is_ok());
                    result
                })
                .await;
            result?;
            deliveries = deliveries.saturating_add(1);
        }
        Ok(IrqDeliverySummary { deliveries })
    }
}

impl IrqDeliveryHandle {
    /// Requests host delivery shutdown without changing device IRQ state.
    pub fn request_shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    /// Returns host handler lifecycle events in deterministic delivery order.
    pub async fn trace_events(&self) -> Vec<TraceEvent> {
        self.trace.lock().await.events().to_vec()
    }
}

impl IrqDeliverySummary {
    /// Number of notifications whose handler turns completed successfully.
    pub const fn deliveries(self) -> usize {
        self.deliveries
    }
}
