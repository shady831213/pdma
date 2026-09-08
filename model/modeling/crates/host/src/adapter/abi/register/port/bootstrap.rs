use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use pdma_model_contract::{
    abi::{QueueRingLayouts, registers::QueueRegisters},
    semantic::QueueId,
};

pub(super) async fn stage_activation<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    queue: QueueId,
    rings: QueueRingLayouts,
) -> PdmaPortResult<()> {
    for (offset, value) in [
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
    ] {
        registers::write(port, registers::queue_offset(queue, offset)?, value).await?;
    }
    Ok(())
}
