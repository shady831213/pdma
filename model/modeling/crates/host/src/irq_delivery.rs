//! Host-owned IRQ delivery actor.

mod trace;

use crate::{AsyncDevicePort, SoftwareCpu};
use pdma_model_contract::{IrqNotification, IrqVector, ModelError, ModelResult, TraceEvent};
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

/// Type-state marker for a delivery actor without a registered handler.
#[derive(Debug)]
pub struct NoHandler(PhantomData<()>);

/// Type-state marker carrying a non-empty vector-to-handler registry.
pub struct HasHandlers<'handler> {
    handlers: BTreeMap<IrqVector, Box<dyn IrqHandler + 'handler>>,
}

impl fmt::Debug for HasHandlers<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HasHandlers")
            .field("vectors", &self.handlers.keys().collect::<Vec<_>>())
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
        let handle = IrqDeliveryHandle {
            shutdown: self.shutdown_sender.clone(),
            trace: Arc::clone(&self.trace),
        };
        let mut handlers: BTreeMap<IrqVector, Box<dyn IrqHandler + 'handler>> = BTreeMap::new();
        handlers.insert(vector, Box::new(handler));
        (
            IrqDelivery {
                port: self.port,
                cpu: self.cpu,
                handler: HasHandlers { handlers },
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
        match self.handler.handlers.entry(vector) {
            Entry::Vacant(entry) => {
                entry.insert(Box::new(handler));
                Ok(())
            }
            Entry::Occupied(_) => Err(ModelError::DuplicateIrqVector(vector)),
        }
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
