//! Exact-claim source probe and cancellation schedules.

#[path = "source/adverse.rs"]
mod adverse;

use super::lock;
use hardware_interface_runtime::{AccessOrigin, RegisterObservation};
use pdma_model_contract::abi::mmio::MmioAccessError;
use pdma_model_contract::semantic::{
    EpochIrqSource, IrqMessage, IrqMessageId, IrqNotification, IrqVector, QueueConfigHandle,
};
use pdma_model_host::register::{PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim};
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
    transcript_lengths: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Stable source state after readiness and claims.
pub struct SourceSnapshot {
    /// Successfully claimed notification count.
    pub notifications: usize,
    /// Empty revalidation count.
    pub empty: usize,
    /// Exact observation cardinality of each compound claim.
    pub transcript_lengths: Vec<usize>,
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
                transcript_lengths: Vec::new(),
            })),
        }
    }

    pub(crate) fn snapshot(&self) -> SourceSnapshot {
        let state = lock(&self.state);
        SourceSnapshot {
            notifications: state.notifications,
            empty: state.empty,
            transcript_lengths: state.transcript_lengths.clone(),
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
            PdmaClaimResult::Notification { observations, .. } => {
                state.notifications = state.notifications.saturating_add(1);
                state.transcript_lengths.push(observations.len());
            }
            PdmaClaimResult::Empty => {
                state.empty = state.empty.saturating_add(1);
                state.transcript_lengths.push(2);
            }
            PdmaClaimResult::Shutdown => {}
        }
        drop(state);
        Ok(result)
    }
}

fn notification(vector: IrqVector, source: EpochIrqSource, id: usize) -> PdmaClaimResult {
    PdmaClaimResult::Notification {
        notification: IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(id),
            vector,
            source,
        )),
        observations: exact_claim_observations(),
    }
}

fn exact_claim_observations() -> Vec<RegisterObservation> {
    let reads: [(u32, &[u8]); 10] = [
        (0x224, &[1, 0, 0, 0]),
        (0x240, &[0; 8]),
        (0x248, &[0; 8]),
        (0x250, &[0; 4]),
        (0x254, &[0x4b, 0, 0, 0]),
        (0x258, &[1, 0, 0, 0]),
        (0x25c, &[2, 0, 0, 0]),
        (0x260, &[0x29, 0, 0, 0, 0, 0, 0, 0]),
        (0x268, &[0; 8]),
        (0x270, &[0; 8]),
    ];
    let mut observations = vec![RegisterObservation::write(
        0,
        AccessOrigin::Direct,
        0x220,
        &[9, 0, 0, 0],
    )];
    observations.extend(
        reads
            .into_iter()
            .enumerate()
            .map(|(index, (offset, bytes))| {
                RegisterObservation::read(index as u64 + 1, AccessOrigin::Direct, offset, bytes)
            }),
    );
    observations
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
