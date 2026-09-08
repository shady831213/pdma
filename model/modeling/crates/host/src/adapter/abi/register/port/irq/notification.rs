use super::super::registers::irq_offset;
use super::super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::PortError;
use pdma_model_contract::abi::mmio::{
    IrqRegisters, IrqResponseRegisters, IrqResponseRegistersView, IrqSourceStatus,
    ResponseCodecError,
};
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, EpochIrqSource, IrqCause, IrqLine, IrqMessage, IrqMessageId,
    IrqNotification, IrqSnapshot, IrqSource, IrqVector, QueueId,
};

pub(super) async fn parse_notification<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
) -> PdmaPortResult<IrqNotification> {
    let offsets = IrqResponseRegisters::SPEC
        .placements()
        .iter()
        .map(|placement| irq_offset(IrqRegisters::RESPONSE.offset() + placement.offset()));
    let bytes = registers::read_bank(port, offsets).await?;
    let view = IrqResponseRegistersView::new(&bytes).map_err(ResponseCodecError::from)?;
    let epoch = DeviceEpoch::new(
        view.epoch()
            .map_err(ResponseCodecError::from)?
            .value()
            .map_err(ResponseCodecError::from)?,
    );
    let source_id = usize::try_from(
        view.source_id()
            .map_err(ResponseCodecError::from)?
            .value()
            .map_err(ResponseCodecError::from)?,
    )
    .map_err(|_| PortError::SubmissionState)?;
    let source_kind = view
        .source_kind()
        .map_err(ResponseCodecError::from)?
        .value()
        .map_err(ResponseCodecError::from)?;
    let source = match source_kind {
        0 => IrqSource::Completion(CompletionRingId::new(source_id)),
        1 => IrqSource::Event(QueueId::new(source_id)),
        _ => return Err(PortError::SubmissionState.into()),
    };
    let state = u32::try_from(
        view.state()
            .map_err(ResponseCodecError::from)?
            .value()
            .map_err(ResponseCodecError::from)?,
    )
    .map_err(|_| PortError::SubmissionState)?;
    IrqSourceStatus::try_from(state).map_err(|_| PortError::SubmissionState)?;
    let route_kind = view
        .route_kind()
        .map_err(ResponseCodecError::from)?
        .value()
        .map_err(ResponseCodecError::from)?;
    let notification_kind = view
        .notification_kind()
        .map_err(ResponseCodecError::from)?
        .value()
        .map_err(ResponseCodecError::from)?;
    let vector = IrqVector::new(
        usize::try_from(
            view.vector()
                .map_err(ResponseCodecError::from)?
                .value()
                .map_err(ResponseCodecError::from)?,
        )
        .map_err(|_| PortError::SubmissionState)?,
    );
    let message_id = usize::try_from(
        view.message_id()
            .map_err(ResponseCodecError::from)?
            .value()
            .map_err(ResponseCodecError::from)?,
    )
    .map_err(|_| PortError::SubmissionState)?;
    let source = EpochIrqSource::new(epoch, source);
    match (notification_kind, route_kind) {
        (1, 0) if message_id == 0 => Ok(IrqNotification::Line(IrqSnapshot::new(
            IrqCause::new(IrqLine::new(vector), source),
            true,
        ))),
        (2, 1) => Ok(IrqNotification::Message(IrqMessage::new(
            IrqMessageId::new(message_id),
            vector,
            source,
        ))),
        _ => Err(PortError::SubmissionState.into()),
    }
}
