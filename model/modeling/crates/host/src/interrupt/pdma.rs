use crate::register::{PdmaClaimResult, PdmaExactInterruptClaim};
use crate::{AsyncDriver, DriverIrqRecord, DriverPort, ModelError};
use hardware_interface_runtime::{
    FrozenHandlerRegistry, InterruptDispatcher, InterruptDispatcherTask, InterruptEnvelope,
    InterruptHandler, InterruptSource, RegisterObservation, SharedInterruptObserver, SoftwareCpu,
};
use pdma_model_contract::{IrqNotification, IrqVector};
use std::{error::Error, fmt, future::Future, sync::Arc};

/// Affine proof that a PDMA interrupt source reported readiness.
#[derive(Debug)]
pub struct PdmaInterruptReady(());

/// One atomically claimed PDMA notification and its exact register transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdmaInterruptNotification {
    notification: IrqNotification,
    observations: Vec<RegisterObservation>,
}

impl PdmaInterruptNotification {
    /// Returns the semantic notification captured by the exact claim.
    pub const fn notification(&self) -> IrqNotification {
        self.notification
    }

    /// Returns the exact Direct observations emitted by the compound claim.
    pub fn observations(&self) -> &[RegisterObservation] {
        &self.observations
    }
}

impl InterruptEnvelope for PdmaInterruptNotification {
    type Vector = IrqVector;

    fn vector(&self) -> Self::Vector {
        self.notification.vector()
    }
}

/// Terminal failure from the PDMA readiness or exact-claim adapter.
#[derive(Debug)]
pub enum PdmaInterruptSourceError<E> {
    /// The device interrupt source closed.
    Closed,
    /// The canonical exact-claim capability failed.
    Access(E),
}

impl<E> fmt::Display for PdmaInterruptSourceError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("PDMA interrupt source closed"),
            Self::Access(_) => formatter.write_str("PDMA interrupt source access failed"),
        }
    }
}

impl<E: Error + 'static> Error for PdmaInterruptSourceError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Closed => None,
            Self::Access(source) => Some(source),
        }
    }
}

/// PDMA adapter for the generic two-phase runtime interrupt source.
#[derive(Clone, Debug)]
pub struct PdmaInterruptSource<A> {
    access: A,
}

impl<A> PdmaInterruptSource<A> {
    /// Creates a source over the one canonical exact-claim capability.
    pub const fn new(access: A) -> Self {
        Self { access }
    }
}

impl<A> InterruptSource for PdmaInterruptSource<A>
where
    A: PdmaExactInterruptClaim + Send + 'static,
{
    type Notification = PdmaInterruptNotification;
    type Ready = PdmaInterruptReady;
    type Error = PdmaInterruptSourceError<A::Error>;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        self.access
            .wait_interrupt_ready()
            .await
            .map_err(PdmaInterruptSourceError::Access)?
            .then_some(PdmaInterruptReady(()))
            .ok_or(PdmaInterruptSourceError::Closed)
    }

    async fn claim(
        &mut self,
        _ready: Self::Ready,
    ) -> Result<Option<Self::Notification>, Self::Error> {
        match self
            .access
            .claim_interrupt_exact()
            .await
            .map_err(PdmaInterruptSourceError::Access)?
        {
            PdmaClaimResult::Notification {
                notification,
                observations,
            } => Ok(Some(PdmaInterruptNotification {
                notification,
                observations,
            })),
            PdmaClaimResult::Empty => Ok(None),
            PdmaClaimResult::Shutdown => Err(PdmaInterruptSourceError::Closed),
        }
    }
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
    Validation(ModelError),
    /// Completion or event polling failed before ACK.
    Drain(ModelError),
    /// Persistent software processing failed before ACK.
    Handler(E),
    /// ACK failed after every prior phase succeeded.
    Ack(ModelError),
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
            Self::Validation(source) | Self::Drain(source) | Self::Ack(source) => Some(source),
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

impl<P, H> InterruptHandler<PdmaInterruptNotification> for PdmaIrqService<P, H>
where
    P: DriverPort + Send + 'static,
    H: PdmaIrqHandler,
{
    type Error = PdmaIrqServiceError<H::Error>;

    async fn handle(&mut self, claimed: &PdmaInterruptNotification) -> Result<(), Self::Error> {
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
    pub fn start<A, P, H>(
        cpu: Arc<SoftwareCpu>,
        registry: FrozenHandlerRegistry<IrqVector, PdmaInterruptNotification, PdmaIrqService<P, H>>,
        source: PdmaInterruptSource<A>,
        observer: SharedInterruptObserver<PdmaInterruptNotification>,
    ) -> InterruptDispatcherTask<
        IrqVector,
        PdmaInterruptSourceError<A::Error>,
        PdmaIrqServiceError<H::Error>,
    >
    where
        A: PdmaExactInterruptClaim + Send + 'static,
        P: DriverPort + Send + 'static,
        H: PdmaIrqHandler,
    {
        InterruptDispatcher::start(cpu, registry, source, observer)
    }
}
