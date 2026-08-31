//! Host-owned IRQ adapter forwarded to the canonical runtime dispatcher.
#![allow(
    clippy::redundant_pub_crate,
    dead_code,
    unreachable_pub,
    unused_imports,
    reason = "the host IRQ adapter is compiled only by the crate-private compatibility regression harness"
)]

use crate::DriverPort;
use crate::runtime::interrupt::{InterruptError, InterruptResult};
use hardware_interface_runtime::{
    HandlerRegistry, InterruptDispatcher, InterruptShutdown, InterruptShutdownReceiver, SoftwareCpu,
};
use pdma_model_contract::semantic::{IrqVector, TraceEvent};
use std::{
    fmt,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

mod bootstrap;
mod compat;
mod handler;
#[cfg(test)]
mod tests;
mod trace;

pub(crate) use bootstrap::{IrqDeliveryBootstrap, IrqDeliveryRunner};
use compat::{CompatHandler, CompatNotification, CompatObserver, CompatSource};
pub(crate) use handler::{
    IrqHandler, IrqHandlerFuture, StatefulIrqHandler, irq_handler, stateful_irq_handler,
};
use trace::HostIrqTrace;

type RuntimeRegistry<'handler> =
    HandlerRegistry<IrqVector, CompatNotification, CompatHandler<'handler>>;

/// Type-state marker for a delivery adapter without a registered handler.
#[derive(Debug)]
pub(crate) struct NoHandler(PhantomData<()>);

/// Host IRQ adapter registry forwarding duplicate and freeze semantics to the runtime.
pub(crate) struct IrqHandlerRegistry<'handler> {
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
    pub(crate) fn register<H>(&mut self, vector: IrqVector, handler: H) -> InterruptResult<()>
    where
        H: IrqHandler + 'handler,
    {
        self.inner
            .register(vector, CompatHandler::new(handler))
            .map_err(|duplicate| InterruptError::DuplicateVector(*duplicate.vector()))
    }

    fn freeze(self) -> InterruptResult<HasHandlers<'handler>> {
        if self.inner.is_empty() {
            Err(InterruptError::EmptyRegistry)
        } else {
            Ok(HasHandlers {
                registry: self.inner,
            })
        }
    }
}

/// Type-state marker carrying the runtime's frozen registry.
pub(crate) struct HasHandlers<'handler> {
    registry: RuntimeRegistry<'handler>,
}

impl fmt::Debug for HasHandlers<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.registry.fmt(formatter)
    }
}

/// Host IRQ adapter builder whose execution is the canonical runtime dispatcher.
#[derive(Debug)]
pub(crate) struct IrqDelivery<P, H> {
    port: P,
    cpu: SoftwareCpu,
    handler: H,
    shutdown: InterruptShutdown,
    receiver: InterruptShutdownReceiver,
    trace: Arc<Mutex<HostIrqTrace>>,
}

/// Compatibility handle for shutdown and trace observation of the canonical runtime dispatcher.
#[derive(Debug, Clone)]
pub(crate) struct IrqDeliveryHandle {
    shutdown: InterruptShutdown,
    trace: Arc<Mutex<HostIrqTrace>>,
}

/// Final successful-delivery count projected from the runtime summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IrqDeliverySummary {
    deliveries: usize,
}

impl<P> IrqDelivery<P, NoHandler> {
    /// Creates an unregistered host IRQ adapter builder.
    pub(crate) fn new(port: P, cpu: SoftwareCpu) -> Self {
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

    /// Registers the first handler in the host IRQ adapter registry.
    pub(crate) fn register_handler<'handler, H>(
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

impl<'handler, P: DriverPort + 'static> IrqDelivery<P, HasHandlers<'handler>> {
    /// Adds a handler before runtime execution.
    pub(crate) fn register_handler<H>(
        &mut self,
        vector: IrqVector,
        handler: H,
    ) -> InterruptResult<()>
    where
        H: IrqHandler + 'handler,
    {
        self.handler
            .registry
            .register(vector, CompatHandler::new(handler))
            .map_err(|duplicate| InterruptError::DuplicateVector(*duplicate.vector()))
    }

    /// Runs the single canonical runtime dispatcher in this caller-owned future.
    pub(crate) async fn run(self) -> InterruptResult<IrqDeliverySummary> {
        let observer = Arc::new(CompatObserver::new(Arc::clone(&self.trace)));
        let registry = self
            .handler
            .registry
            .freeze()
            .map_err(|_| InterruptError::EmptyRegistry)?;
        let _shutdown = self.shutdown;
        let result = InterruptDispatcher::run(
            Arc::new(self.cpu.clone()),
            registry,
            CompatSource::new(self.port),
            observer,
            self.receiver,
        )
        .await;
        compat::project_result(result, &self.trace)
    }
}

impl IrqDeliveryHandle {
    /// Requests cooperative runtime shutdown.
    pub(crate) fn request_shutdown(&self) {
        self.shutdown.request_shutdown();
    }

    /// Returns runtime-observed lifecycle events.
    pub(crate) async fn trace_events(&self) -> Vec<TraceEvent> {
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
    pub(crate) const fn deliveries(self) -> usize {
        self.deliveries
    }
}
