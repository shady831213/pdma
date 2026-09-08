use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, control, registers};
use crate::{PortError, adapter::abi::RegisterQueueStatus, semantic::SubmissionReceipt};
use pdma_model_contract::{
    abi::{
        QueueRingLayouts,
        mmio::{MmioResult, RegisterId},
    },
    semantic::{CompletionRecord, EpochQueue, EventRecord, QueueConfigHandle},
};

#[derive(Debug, Clone)]
pub(super) enum Effect {
    Status,
    Activate {
        handle: QueueConfigHandle,
        rings: QueueRingLayouts,
    },
    Submit(Option<SubmissionReceipt>),
    Completion(CompletionRecord),
    Event(EventRecord),
}

impl Effect {
    pub(super) const fn receipt(&self) -> Option<&SubmissionReceipt> {
        match self {
            Self::Submit(receipt) => receipt.as_ref(),
            Self::Status | Self::Activate { .. } | Self::Completion(_) | Self::Event(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PendingCommand {
    pub(super) queue: EpochQueue,
    pub(super) rings: QueueRingLayouts,
    pub(super) effect: Effect,
}

pub(super) async fn recover<A: CanonicalAccess>(port: &PdmaRegisterPort<A>) -> PdmaPortResult<()> {
    resolve(port).await.map(|_| ())
}

pub(super) async fn resolve<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
) -> PdmaPortResult<Option<RegisterQueueStatus>> {
    let Some(pending) = port.with_state(|state| state.pending.clone())? else {
        return Ok(None);
    };
    let result = u32::try_from(registers::read(port, RegisterId::QResult).await?)
        .map_err(|_| PortError::SubmissionState)?;
    let result = MmioResult::try_from(result).map_err(|_| PortError::SubmissionState)?;
    if result != MmioResult::Ok {
        if result.is_error()
            && let Some(receipt) = pending.effect.receipt()
        {
            receipt.reject();
        }
        let error = control::map_result(port, result.code(), pending.queue).await?;
        port.with_state(|state| state.pending = None)?;
        return Err(error.into());
    }
    let status = control::decode_status(
        &registers::read_bank(port, &control::QUEUE_RESPONSE).await?,
        pending.rings,
    )?;
    let epoch = registers::read(port, RegisterId::QRspEpoch).await?;
    let result = registers::read(port, RegisterId::QResult).await?;
    if result != u64::from(MmioResult::Ok.code())
        || status.queue() != pending.queue
        || epoch != pending.queue.epoch().raw()
    {
        return Err(PortError::SubmissionState.into());
    }
    port.with_state(|state| {
        if let Some(queue) = state.queue.as_mut() {
            queue.status = status;
        }
        match pending.effect {
            Effect::Status | Effect::Submit(_) => {}
            Effect::Activate { handle, rings } => {
                state.queue = Some(super::state::QueueState {
                    handle,
                    rings,
                    status,
                });
                state.activation = Some(handle);
            }
            Effect::Completion(record) => state.completion = Some(record),
            Effect::Event(record) => state.event = Some(record),
        }
        state.pending = None;
    })?;
    Ok(Some(status))
}
