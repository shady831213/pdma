use super::registers::queue_offset;
use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, control, registers};
use crate::PortError;
use pdma_model_contract::{
    abi::{QueueRingLayouts, mmio::QueueRegisters},
    semantic::QueueActivation,
};

pub(super) async fn stage_activation<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    activation: QueueActivation,
    rings: QueueRingLayouts,
) -> PdmaPortResult<()> {
    control::stage_queue(port, activation.queue()).await?;
    let config = activation.config();
    let values = [
        (
            QueueRegisters::CFG_SQ_DEPTH.offset(),
            u64::from(rings.submission().depth()),
        ),
        (
            QueueRegisters::CFG_CQ_DEPTH.offset(),
            u64::from(rings.completion().depth()),
        ),
        (QueueRegisters::CFG_EVENT_POLICY.offset(), 0),
        (
            QueueRegisters::CFG_SOFTWARE_OWNER.offset(),
            u64::try_from(config.software_owner().raw()).map_err(|_| PortError::SubmissionState)?,
        ),
        (
            QueueRegisters::SQ_BASE.offset(),
            rings.submission().region().base().raw(),
        ),
        (
            QueueRegisters::SQ_DEPTH.offset(),
            u64::from(rings.submission().depth()),
        ),
        (
            QueueRegisters::CQ_BASE.offset(),
            rings.completion().region().base().raw(),
        ),
        (
            QueueRegisters::CQ_DEPTH.offset(),
            u64::from(rings.completion().depth()),
        ),
        (
            QueueRegisters::EQ_BASE.offset(),
            rings.event().region().base().raw(),
        ),
        (
            QueueRegisters::EQ_DEPTH.offset(),
            u64::from(rings.event().depth()),
        ),
    ];
    for (offset, value) in values {
        registers::write(port, queue_offset(offset), value).await?;
    }
    Ok(())
}
