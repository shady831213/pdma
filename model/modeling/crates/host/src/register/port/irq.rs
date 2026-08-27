use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::PortError;
use pdma_model_contract::abi::mmio::{
    IrqCommand, IrqResponseRegistersView, IrqSourceStatus, MmioResult, RegisterId,
    ResponseCodecError,
};
use pdma_model_contract::semantic::{
    CompletionRingId, DeviceEpoch, EpochIrqSource, IrqCause, IrqLine, IrqMessage, IrqMessageId,
    IrqNotification, IrqRoute, IrqSnapshot, IrqSource, IrqVector, QueueId,
};

const IRQ_RESPONSE: [RegisterId; 9] = [
    RegisterId::IRspEpoch,
    RegisterId::IRspSourceId,
    RegisterId::IRspSourceKind,
    RegisterId::IRspState,
    RegisterId::IRspRouteKind,
    RegisterId::IRspNotificationKind,
    RegisterId::IRspVector,
    RegisterId::IRspMessageId,
    RegisterId::IRspPendingMessages,
];

pub(super) async fn source_command<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    source: EpochIrqSource,
    route: Option<IrqRoute>,
    command: IrqCommand,
) -> PdmaPortResult<()> {
    validate_source(port, source)?;
    let _window = port.irq_window.lock().await;
    registers::write(port, RegisterId::IExpectedEpoch, source.epoch().raw()).await?;
    let (kind, id) = encode_source(source.source())?;
    registers::write(port, RegisterId::ISourceId, id).await?;
    registers::write(port, RegisterId::ISourceKind, u64::from(kind)).await?;
    if let Some(route) = route {
        let (kind, vector) = encode_route(route)?;
        registers::write(port, RegisterId::IRouteKind, u64::from(kind)).await?;
        registers::write(port, RegisterId::IRouteVector, vector).await?;
    }
    registers::write(port, RegisterId::ICommand, u64::from(command.code())).await?;
    let result = u32::try_from(registers::read(port, RegisterId::IResult).await?)
        .map_err(|_| PortError::SubmissionState)?;
    match MmioResult::try_from(result) {
        Ok(MmioResult::Ok) => {
            let state = u32::try_from(registers::read(port, RegisterId::IRspState).await?)
                .map_err(|_| PortError::SubmissionState)?;
            IrqSourceStatus::try_from(state).map_err(|_| PortError::SubmissionState)?;
            Ok(())
        }
        Ok(MmioResult::StaleEpoch) => Err(PortError::StaleEpoch {
            expected: DeviceEpoch::new(registers::read(port, RegisterId::IDetail0).await?),
            actual: DeviceEpoch::new(registers::read(port, RegisterId::IDetail1).await?),
        }
        .into()),
        Ok(MmioResult::InvalidIrqSource) => {
            Err(PortError::UnknownIrqSource(source.source()).into())
        }
        Ok(MmioResult::IrqSourceActive) => Err(PortError::IrqSourceActive(source.source()).into()),
        _ => Err(PortError::SubmissionState.into()),
    }
}

pub(super) async fn snapshot<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
) -> PdmaPortResult<Option<IrqSnapshot>> {
    let _window = port.irq_window.lock().await;
    registers::write(
        port,
        RegisterId::ICommand,
        u64::from(IrqCommand::SnapshotLine.code()),
    )
    .await?;
    let result = u32::try_from(registers::read(port, RegisterId::IResult).await?)
        .map_err(|_| PortError::SubmissionState)?;
    if MmioResult::try_from(result) != Ok(MmioResult::Ok) {
        return Ok(None);
    }
    match parse_notification(port).await? {
        IrqNotification::Line(snapshot) => Ok(Some(snapshot)),
        IrqNotification::Message(_) => Ok(None),
    }
}

pub(super) async fn wait_line<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
) -> PdmaPortResult<Option<IrqSnapshot>> {
    loop {
        if !port.poll_claim(port.access.wait_interrupt_ready()).await? {
            return Ok(None);
        }
        if let Some(snapshot) = snapshot(port).await? {
            return Ok(Some(snapshot));
        }
    }
}

async fn parse_notification<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
) -> PdmaPortResult<IrqNotification> {
    let bytes = registers::read_bank(port, &IRQ_RESPONSE).await?;
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

fn validate_source<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    source: EpochIrqSource,
) -> PdmaPortResult<()> {
    Ok(port.with_state(|state| {
        let queue = state
            .queue
            .ok_or_else(|| PortError::UnknownIrqSource(source.source()))?;
        if source.epoch() != queue.handle.epoch() {
            return Err(PortError::StaleEpoch {
                expected: queue.handle.epoch(),
                actual: source.epoch(),
            });
        }
        if source != queue.handle.completion_irq_source()
            && source != queue.handle.event_irq_source()
        {
            return Err(PortError::UnknownIrqSource(source.source()));
        }
        Ok(())
    })??)
}

fn encode_source(source: IrqSource) -> PdmaPortResult<(u32, u64)> {
    match source {
        IrqSource::Completion(id) => u64::try_from(id.raw()).map(|id| (0, id)),
        IrqSource::Event(id) => u64::try_from(id.raw()).map(|id| (1, id)),
    }
    .map_err(|_| PortError::SubmissionState.into())
}

fn encode_route(route: IrqRoute) -> PdmaPortResult<(u32, u64)> {
    match route {
        IrqRoute::Line(line) => u64::try_from(line.vector().raw()).map(|vector| (0, vector)),
        IrqRoute::Message(vector) => u64::try_from(vector.raw()).map(|vector| (1, vector)),
    }
    .map_err(|_| PortError::SubmissionState.into())
}
