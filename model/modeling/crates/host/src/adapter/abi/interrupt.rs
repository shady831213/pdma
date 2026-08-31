use super::{PdmaClaimResult, PdmaExactInterruptClaim};
use crate::runtime::interrupt::PdmaNotificationEnvelope;
use hardware_interface_runtime::{InterruptEnvelope, InterruptSource, RegisterObservation};
use pdma_model_contract::semantic::{IrqNotification, IrqVector};
use std::{error::Error, fmt};

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

impl PdmaNotificationEnvelope for PdmaInterruptNotification {
    fn notification(&self) -> IrqNotification {
        self.notification
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
