mod notification;

use super::registers::irq_offset;
use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::PortError;
use pdma_model_contract::abi::mmio::{
    IrqCommand, IrqRegisters, IrqResponseRegisters, IrqSourceStatus, MmioResult,
};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochIrqSource, IrqNotification, IrqRoute, IrqSnapshot, IrqSource,
};

pub(super) async fn source_command<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    source: EpochIrqSource,
    route: Option<IrqRoute>,
    command: IrqCommand,
) -> PdmaPortResult<()> {
    validate_source(port, source)?;
    let mut window = port.access.irq_coordinator().lock().await;
    port.poll_claim(window.recover(&port.access)).await?;
    registers::write(
        port,
        irq_offset(IrqRegisters::EXPECTED_EPOCH.offset()),
        source.epoch().raw(),
    )
    .await?;
    let (kind, id) = encode_source(source.source())?;
    registers::write(port, irq_offset(IrqRegisters::SOURCE_ID.offset()), id).await?;
    registers::write(
        port,
        irq_offset(IrqRegisters::SOURCE_KIND.offset()),
        u64::from(kind),
    )
    .await?;
    if let Some(route) = route {
        let (kind, vector) = encode_route(route)?;
        registers::write(
            port,
            irq_offset(IrqRegisters::ROUTE_KIND.offset()),
            u64::from(kind),
        )
        .await?;
        registers::write(
            port,
            irq_offset(IrqRegisters::ROUTE_VECTOR.offset()),
            vector,
        )
        .await?;
    }
    registers::write(
        port,
        irq_offset(IrqRegisters::COMMAND.offset()),
        u64::from(command.code()),
    )
    .await?;
    let result =
        u32::try_from(registers::read(port, irq_offset(IrqRegisters::RESULT.offset())).await?)
            .map_err(|_| PortError::SubmissionState)?;
    let outcome = match MmioResult::try_from(result) {
        Ok(MmioResult::Ok) => {
            let state = u32::try_from(
                registers::read(
                    port,
                    irq_offset(
                        IrqRegisters::RESPONSE.offset() + IrqResponseRegisters::STATE.offset(),
                    ),
                )
                .await?,
            )
            .map_err(|_| PortError::SubmissionState)?;
            IrqSourceStatus::try_from(state).map_err(|_| PortError::SubmissionState)?;
            Ok(())
        }
        Ok(MmioResult::StaleEpoch) => Err(PortError::StaleEpoch {
            expected: DeviceEpoch::new(
                registers::read(port, irq_offset(IrqRegisters::DETAIL0.offset())).await?,
            ),
            actual: DeviceEpoch::new(
                registers::read(port, irq_offset(IrqRegisters::DETAIL1.offset())).await?,
            ),
        }
        .into()),
        Ok(MmioResult::InvalidIrqSource) => {
            Err(PortError::UnknownIrqSource(source.source()).into())
        }
        Ok(MmioResult::IrqSourceActive) => Err(PortError::IrqSourceActive(source.source()).into()),
        _ => Err(super::PdmaClaimError::CommandResult {
            code: u64::from(result),
        }
        .into()),
    };
    drop(window);
    outcome
}

pub(super) async fn snapshot<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
) -> PdmaPortResult<Option<IrqSnapshot>> {
    let mut window = port.access.irq_coordinator().lock().await;
    port.poll_claim(window.recover(&port.access)).await?;
    registers::write(
        port,
        irq_offset(IrqRegisters::COMMAND.offset()),
        u64::from(IrqCommand::SnapshotLine.code()),
    )
    .await?;
    let result =
        u32::try_from(registers::read(port, irq_offset(IrqRegisters::RESULT.offset())).await?)
            .map_err(|_| PortError::SubmissionState)?;
    match MmioResult::try_from(result) {
        Ok(MmioResult::Ok) => {}
        Ok(MmioResult::Empty) => return Ok(None),
        _ => {
            return Err(super::PdmaClaimError::CommandResult {
                code: u64::from(result),
            }
            .into());
        }
    }
    let outcome = match notification::parse_notification(port).await? {
        IrqNotification::Line(snapshot) => Ok(Some(snapshot)),
        IrqNotification::Message(_) => Ok(None),
    };
    drop(window);
    outcome
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
