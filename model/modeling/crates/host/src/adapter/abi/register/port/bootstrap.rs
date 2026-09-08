use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, control, registers};
use crate::PortError;
use pdma_model_contract::{
    abi::{QueueRingLayouts, mmio::RegisterId},
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
            RegisterId::QCfgSqDepth,
            u64::from(rings.submission().depth()),
        ),
        (
            RegisterId::QCfgCqDepth,
            u64::from(rings.completion().depth()),
        ),
        (RegisterId::QCfgEventPolicy, 0),
        (
            RegisterId::QCfgSoftwareOwner,
            u64::try_from(config.software_owner().raw()).map_err(|_| PortError::SubmissionState)?,
        ),
        (
            RegisterId::QSqBase,
            rings.submission().region().base().raw(),
        ),
        (RegisterId::QSqDepth, u64::from(rings.submission().depth())),
        (
            RegisterId::QCqBase,
            rings.completion().region().base().raw(),
        ),
        (RegisterId::QCqDepth, u64::from(rings.completion().depth())),
        (RegisterId::QEqBase, rings.event().region().base().raw()),
        (RegisterId::QEqDepth, u64::from(rings.event().depth())),
    ];
    for (id, value) in values {
        registers::write(port, id, value).await?;
    }
    Ok(())
}
