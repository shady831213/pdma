//! Exact-claim source probe and cancellation schedules.

mod adverse;

use super::lock;
use pdma_model_contract::abi::mmio::MmioAccessError;
use pdma_model_contract::semantic::{
    EpochIrqSource, IrqMessage, IrqMessageId, IrqNotification, IrqVector, QueueConfigHandle,
};
use pdma_model_host::adapter::abi::{PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
/// Clone-shared exact interrupt-claim capability.
pub struct ProbeAccess {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    claims: VecDeque<PdmaClaimResult>,
    ready_error: bool,
    claim_error: bool,
    pend_claim: bool,
    notifications: usize,
    empty: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Stable source state after readiness and claims.
pub struct SourceSnapshot {
    /// Successfully claimed notification count.
    pub notifications: usize,
    /// Empty revalidation count.
    pub empty: usize,
}

impl ProbeAccess {
    pub(crate) fn success(vector: IrqVector, queue: QueueConfigHandle) -> Self {
        Self::from_claims([
            notification(vector, queue.completion_irq_source(), 0),
            PdmaClaimResult::Empty,
            notification(vector, queue.event_irq_source(), 1),
            notification(vector, queue.completion_irq_source(), 2),
        ])
    }

    fn one(vector: IrqVector, source: EpochIrqSource) -> Self {
        Self::from_claims([notification(vector, source, 0)])
    }

    fn from_claims(claims: impl IntoIterator<Item = PdmaClaimResult>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                claims: claims.into_iter().collect(),
                ready_error: false,
                claim_error: false,
                pend_claim: false,
                notifications: 0,
                empty: 0,
            })),
        }
    }

    pub(crate) fn snapshot(&self) -> SourceSnapshot {
        let state = lock(&self.state);
        SourceSnapshot {
            notifications: state.notifications,
            empty: state.empty,
        }
    }
}

impl PdmaExactInterruptClaim for ProbeAccess {
    type Error = PdmaClaimError;

    async fn wait_interrupt_ready(&self) -> Result<bool, Self::Error> {
        if lock(&self.state).ready_error {
            return Err(probe_error());
        }
        while lock(&self.state).claims.is_empty() {
            tokio::task::yield_now().await;
        }
        Ok(true)
    }

    async fn claim_interrupt_exact(&self) -> Result<PdmaClaimResult, Self::Error> {
        if lock(&self.state).pend_claim {
            tokio::task::yield_now().await;
        }
        let mut state = lock(&self.state);
        if state.claim_error {
            return Err(probe_error());
        }
        let result = state.claims.pop_front().unwrap_or(PdmaClaimResult::Empty);
        match &result {
            PdmaClaimResult::Notification { .. } => {
                state.notifications = state.notifications.saturating_add(1);
            }
            PdmaClaimResult::Empty => {
                state.empty = state.empty.saturating_add(1);
            }
            PdmaClaimResult::Shutdown => {}
        }
        drop(state);
        Ok(result)
    }
}

const fn notification(vector: IrqVector, source: EpochIrqSource, id: usize) -> PdmaClaimResult {
    PdmaClaimResult::Notification {
        notification: IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(id),
            vector,
            source,
        )),
    }
}

const fn probe_error() -> PdmaClaimError {
    PdmaClaimError::Access(MmioAccessError::WrongWidth {
        offset: 0x220,
        expected: 4,
        actual: 8,
    })
}

pub(super) async fn run_adverse() -> super::AdverseFacts {
    adverse::run_adverse().await
}
