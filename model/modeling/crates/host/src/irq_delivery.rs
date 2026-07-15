//! Host-owned IRQ delivery actor.

mod trace;

use crate::{AsyncDevicePort, SoftwareCpu};
use pdma_model_contract::{IrqNotification, ModelResult, TraceEvent};
use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};
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

    /// Registers the single PDMA software handler and makes the actor runnable.
    pub fn register_handler<H>(self, handler: H) -> (IrqDelivery<P, H>, IrqDeliveryHandle)
    where
        H: IrqHandler,
    {
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

impl<P, H> IrqDelivery<P, H>
where
    P: AsyncDevicePort,
    H: IrqHandler,
{
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
            let trace = Arc::clone(&self.trace);
            let handler = &mut self.handler;
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
