use super::{HostIrqTrace, IrqDeliverySummary, IrqHandler};
use crate::{AsyncDevicePort, ModelError, ModelResult};
use hardware_interface_runtime::{
    InterruptDispatchError, InterruptEnvelope, InterruptHandler, InterruptLifecycleEvent,
    InterruptLifecycleObserver, InterruptSource, InterruptSummary,
};
use pdma_model_contract::{IrqNotification, IrqVector};
use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub(super) struct CompatNotification(IrqNotification);

impl InterruptEnvelope for CompatNotification {
    type Vector = IrqVector;

    fn vector(&self) -> Self::Vector {
        self.0.vector()
    }
}

pub(super) struct CompatHandler<'handler>(Box<dyn IrqHandler + 'handler>);

impl<'handler> CompatHandler<'handler> {
    pub(super) fn new(handler: impl IrqHandler + 'handler) -> Self {
        Self(Box::new(handler))
    }
}

impl InterruptHandler<CompatNotification> for CompatHandler<'_> {
    type Error = ModelError;

    async fn handle(&mut self, notification: &CompatNotification) -> ModelResult<()> {
        self.0.handle(notification.0).await
    }
}

pub(super) struct CompatSource<P> {
    port: tokio::sync::Mutex<P>,
}

impl<P> CompatSource<P> {
    pub(super) const fn new(port: P) -> Self {
        Self {
            port: tokio::sync::Mutex::const_new(port),
        }
    }
}

#[derive(Debug)]
pub(super) struct CompatSourceClosed;

impl fmt::Display for CompatSourceClosed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("compatibility interrupt source closed")
    }
}

impl Error for CompatSourceClosed {}

impl<P: AsyncDevicePort + 'static> InterruptSource for CompatSource<P> {
    type Notification = CompatNotification;
    type Ready = CompatNotification;
    type Error = CompatSourceClosed;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        self.port
            .lock()
            .await
            .wait_irq_notification()
            .await
            .map(CompatNotification)
            .ok_or(CompatSourceClosed)
    }

    async fn claim(
        &mut self,
        ready: Self::Ready,
    ) -> Result<Option<Self::Notification>, Self::Error> {
        Ok(Some(ready))
    }
}

pub(super) struct CompatObserver {
    trace: Arc<Mutex<HostIrqTrace>>,
}

impl CompatObserver {
    pub(super) const fn new(trace: Arc<Mutex<HostIrqTrace>>) -> Self {
        Self { trace }
    }
}

impl InterruptLifecycleObserver<CompatNotification> for CompatObserver {
    fn observe(&self, event: InterruptLifecycleEvent<'_, CompatNotification>) {
        let mut trace = self
            .trace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match event {
            InterruptLifecycleEvent::Started { notification, .. } => {
                trace.handler_started(notification.0);
            }
            InterruptLifecycleEvent::Succeeded { .. } => trace.handler_completed(true),
            InterruptLifecycleEvent::Failed { .. } => trace.handler_completed(false),
        }
    }
}

pub(super) fn project_result(
    result: &Result<
        InterruptSummary,
        InterruptDispatchError<IrqVector, CompatSourceClosed, ModelError>,
    >,
    trace: &Mutex<HostIrqTrace>,
) -> ModelResult<IrqDeliverySummary> {
    match result {
        Ok(summary) => Ok(IrqDeliverySummary {
            deliveries: summary.deliveries(),
        }),
        Err(InterruptDispatchError::Readiness(CompatSourceClosed)) => Ok(IrqDeliverySummary {
            deliveries: trace
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .successful_deliveries(),
        }),
        Err(InterruptDispatchError::UnregisteredVector(vector)) => {
            Err(ModelError::UnregisteredIrqVector(*vector))
        }
        Err(InterruptDispatchError::Handler { source, .. }) => Err(*source),
        Err(InterruptDispatchError::Claim(CompatSourceClosed)) => {
            Err(ModelError::IrqDeliveryUnavailable)
        }
    }
}
