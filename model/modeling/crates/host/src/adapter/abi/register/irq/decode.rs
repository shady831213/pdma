use super::super::{PdmaClaimError, PdmaRegisterAccessError};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    abi::mmio::{
        IrqCommand, IrqRegisters, IrqResponseRegisters, IrqSourceStatus, PdmaMmioMap, register_at,
    },
    semantic::{
        CompletionRingId, DeviceEpoch, EpochIrqSource, IrqCause, IrqLine, IrqMessage, IrqMessageId,
        IrqNotification, IrqSnapshot, IrqSource, IrqVector, QueueId,
    },
};

pub(super) fn response_offsets() -> impl Iterator<Item = usize> {
    IrqResponseRegisters::SPEC
        .placements()
        .iter()
        .map(|placement| IrqRegisters::RESPONSE.offset() + placement.offset())
}

pub(super) async fn read<A: RegisterAccess<Error = PdmaRegisterAccessError> + Sync>(
    access: &A,
    local: usize,
) -> Result<u64, PdmaClaimError> {
    let register =
        register_at(PdmaMmioMap::IRQ.offset() + local).ok_or(PdmaClaimError::MalformedResponse)?;
    let offset = u32::try_from(register.offset()).map_err(|_| PdmaClaimError::MalformedResponse)?;
    let mut bytes = vec![0; register.width()];
    access
        .read(offset, &mut bytes)
        .await
        .map_err(PdmaClaimError::from)?;
    register
        .definition()
        .view(&bytes)
        .map_err(|_| PdmaClaimError::MalformedResponse)?
        .value()
        .map_err(|_| PdmaClaimError::MalformedResponse)
}

pub(super) async fn write_claim<A: RegisterAccess<Error = PdmaRegisterAccessError> + Sync>(
    access: &A,
    command: IrqCommand,
) -> Result<(), PdmaClaimError> {
    let register = register_at(PdmaMmioMap::IRQ.offset() + IrqRegisters::COMMAND.offset())
        .ok_or(PdmaClaimError::MalformedResponse)?;
    let offset = u32::try_from(register.offset()).map_err(|_| PdmaClaimError::MalformedResponse)?;
    access
        .write(offset, &command.code().to_le_bytes())
        .await
        .map_err(Into::into)
}

pub(super) fn notification(values: &[u64]) -> Result<IrqNotification, PdmaClaimError> {
    let [
        epoch,
        source_id,
        source_kind,
        state,
        route,
        kind,
        vector,
        message,
        _pending,
    ] = values
    else {
        return Err(PdmaClaimError::MalformedResponse);
    };
    let state = u32::try_from(*state).map_err(|_| PdmaClaimError::MalformedResponse)?;
    IrqSourceStatus::try_from(state).map_err(|_| PdmaClaimError::MalformedResponse)?;
    let id = usize::try_from(*source_id).map_err(|_| PdmaClaimError::MalformedResponse)?;
    let source = match source_kind {
        0 => IrqSource::Completion(CompletionRingId::new(id)),
        1 => IrqSource::Event(QueueId::new(id)),
        _ => return Err(PdmaClaimError::MalformedResponse),
    };
    let source = EpochIrqSource::new(DeviceEpoch::new(*epoch), source);
    let vector =
        IrqVector::new(usize::try_from(*vector).map_err(|_| PdmaClaimError::MalformedResponse)?);
    match (*kind, *route) {
        (1, 0) if *message == 0 => Ok(IrqNotification::Line(IrqSnapshot::new(
            IrqCause::new(IrqLine::new(vector), source),
            true,
        ))),
        (2, 1) => Ok(IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(
                usize::try_from(*message).map_err(|_| PdmaClaimError::MalformedResponse)?,
            ),
            vector,
            source,
        ))),
        _ => Err(PdmaClaimError::MalformedResponse),
    }
}
