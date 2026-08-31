use crate::{AsyncDriver, DriverError, DriverIrqRecord, DriverPort, PortError};
use hardware_interface_runtime::{
    FrozenHandlerRegistry, InterruptDispatcher, InterruptDispatcherTask, InterruptEnvelope,
    InterruptHandler, InterruptSource, SharedInterruptObserver, SoftwareCpu,
};
use pdma_model_contract::semantic::{IrqNotification, IrqVector};
use std::{error::Error, fmt, future::Future, sync::Arc};

/// Semantic notification surface consumed by the host IRQ service.
pub trait PdmaNotificationEnvelope: InterruptEnvelope<Vector = IrqVector> {
    /// Returns the semantic notification carried by this runtime envelope.
    fn notification(&self) -> IrqNotification;
}

/// Persistent software logic run after a complete ring drain and before ACK.
pub trait PdmaIrqHandler: Send + 'static {
    /// Typed persistent-handler failure.
    type Error: Error + Send + Sync + 'static;

    /// Processes one drained notification while borrowing persistent state.
    fn handle(
        &mut self,
        notification: IrqNotification,
        records: &[DriverIrqRecord],
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Exact failure phase from one composite PDMA handler turn.
#[derive(Debug)]
pub enum PdmaIrqServiceError<E> {
    /// Epoch or source identity validation failed before any drain.
    Validation(DriverError),
    /// Completion or event polling failed before ACK.
    Drain(DriverError),
    /// Persistent software processing failed before ACK.
    Handler(E),
    /// ACK failed after every prior phase succeeded.
    Ack(PortError),
}

impl<E> fmt::Display for PdmaIrqServiceError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(_) => formatter.write_str("PDMA interrupt validation failed"),
            Self::Drain(_) => formatter.write_str("PDMA interrupt drain failed"),
            Self::Handler(_) => formatter.write_str("PDMA interrupt handler failed"),
            Self::Ack(_) => formatter.write_str("PDMA interrupt ACK failed"),
        }
    }
}

impl<E: Error + 'static> Error for PdmaIrqServiceError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(source) | Self::Drain(source) => Some(source),
            Self::Ack(source) => Some(source),
            Self::Handler(source) => Some(source),
        }
    }
}

/// Persistent composite owner of one driver, port, and software handler state.
#[derive(Debug)]
pub struct PdmaIrqService<P, H> {
    driver: AsyncDriver,
    port: P,
    handler: H,
}

impl<P, H> PdmaIrqService<P, H> {
    /// Binds all await-spanning state to one persistent registry entry.
    pub const fn new(driver: AsyncDriver, port: P, handler: H) -> Self {
        Self {
            driver,
            port,
            handler,
        }
    }

    /// Returns shared accounting and immutable driver identity.
    pub const fn driver(&self) -> &AsyncDriver {
        &self.driver
    }
}

impl<P, H, N> InterruptHandler<N> for PdmaIrqService<P, H>
where
    P: DriverPort + Send + 'static,
    H: PdmaIrqHandler,
    N: PdmaNotificationEnvelope,
{
    type Error = PdmaIrqServiceError<H::Error>;

    async fn handle(&mut self, claimed: &N) -> Result<(), Self::Error> {
        let notification = claimed.notification();
        let source = self
            .driver
            .validate_irq_notification(notification)
            .map_err(PdmaIrqServiceError::Validation)?;
        let records = self
            .driver
            .drain_irq_source_on(&mut self.port, source)
            .await
            .map_err(PdmaIrqServiceError::Drain)?;
        self.handler
            .handle(notification, &records)
            .await
            .map_err(PdmaIrqServiceError::Handler)?;
        self.port
            .ack_irq_source(source)
            .await
            .map_err(PdmaIrqServiceError::Ack)
    }
}

/// Runtime-backed sole-owner PDMA delivery task constructor.
#[derive(Debug, Clone, Copy)]
pub struct PdmaIrqDelivery;

impl PdmaIrqDelivery {
    /// Starts delivery from an already frozen registry.
    pub fn start<S, P, H>(
        cpu: Arc<SoftwareCpu>,
        registry: FrozenHandlerRegistry<IrqVector, S::Notification, PdmaIrqService<P, H>>,
        source: S,
        observer: SharedInterruptObserver<S::Notification>,
    ) -> InterruptDispatcherTask<IrqVector, S::Error, PdmaIrqServiceError<H::Error>>
    where
        S: InterruptSource,
        S::Notification: PdmaNotificationEnvelope,
        P: DriverPort + Send + 'static,
        H: PdmaIrqHandler,
    {
        InterruptDispatcher::start(cpu, registry, source, observer)
    }
}
