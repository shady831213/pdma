use pdma_model_contract::{DeviceEpoch, DmaRegion};
use pdma_model_contract::{
    EpochIrqSource, IrqMessage, IrqMessageId, IrqNotification, IrqSource, IrqVector,
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[path = "recording/transport.rs"]
mod transport;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Access {
    BootstrapBatch,
    SubmissionBatch,
    Read(u32, u128),
    Write(u32, Vec<u8>),
}

#[derive(Clone, Debug)]
pub(super) struct RecordingStage(pub(super) Vec<DmaRegion>);

#[derive(Clone, Debug, Default)]
pub(super) struct RecordingTransport {
    accesses: Arc<Mutex<Vec<Access>>>,
    gate_queue_start: Arc<AtomicBool>,
    queue_started: Arc<Notify>,
    queue_release: Arc<Notify>,
    fail_write: Arc<Mutex<Option<u32>>>,
    batch_failure: Arc<AtomicBool>,
    queue_result: Arc<AtomicU32>,
    notification: Arc<Mutex<Option<IrqNotification>>>,
    claimed: Arc<Mutex<Option<IrqNotification>>>,
    empty_claims: Arc<AtomicU32>,
    irq_command: Arc<AtomicU32>,
}

impl RecordingTransport {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn accesses(&self) -> Vec<Access> {
        self.accesses
            .lock()
            .map_or_else(|_| Vec::new(), |value| value.clone())
    }

    pub(super) fn accesses_json(&self) -> String {
        let entries = self
            .accesses()
            .into_iter()
            .map(|access| match access {
                Access::BootstrapBatch => {
                    String::from("{\"kind\":\"batch\",\"name\":\"bootstrap\"}")
                }
                Access::SubmissionBatch => {
                    String::from("{\"kind\":\"batch\",\"name\":\"submission\"}")
                }
                Access::Read(offset, width) => {
                    format!("{{\"kind\":\"read\",\"offset\":{offset},\"width\":{width}}}")
                }
                Access::Write(offset, bytes) => {
                    format!("{{\"kind\":\"write\",\"offset\":{offset},\"bytes\":{bytes:?}}}")
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    pub(super) fn clear(&self) {
        if let Ok(mut accesses) = self.accesses.lock() {
            accesses.clear();
        }
    }

    pub(super) fn gate_next_queue_start(&self) {
        self.gate_queue_start.store(true, Ordering::SeqCst);
    }

    pub(super) async fn wait_queue_start(&self) {
        self.queue_started.notified().await;
    }

    pub(super) fn fail_next_write(&self, offset: u32) {
        if let Ok(mut failure) = self.fail_write.lock() {
            *failure = Some(offset);
        }
    }

    pub(super) fn fail_next_batch(&self) {
        self.batch_failure.store(true, Ordering::SeqCst);
    }

    pub(super) fn set_queue_result(&self, result: u32) {
        self.queue_result.store(result, Ordering::SeqCst);
    }

    pub(super) fn set_message_notification(&self, vector: usize, id: usize) {
        if let Ok(mut notification) = self.notification.lock() {
            *notification = Some(IrqNotification::Message(IrqMessage::new(
                IrqMessageId::new(id),
                IrqVector::new(vector),
                EpochIrqSource::new(
                    DeviceEpoch::initial(),
                    IrqSource::Completion(pdma_model_contract::CompletionRingId::new(0)),
                ),
            )));
        }
    }

    pub(super) fn empty_next_claim(&self) {
        self.empty_claims.store(1, Ordering::SeqCst);
    }

    fn record(&self, access: Access) {
        if let Ok(mut accesses) = self.accesses.lock() {
            accesses.push(access);
        }
    }
}
