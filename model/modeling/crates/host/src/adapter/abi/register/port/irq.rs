use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort, registers};
use crate::PortError;
use pdma_model_contract::abi::registers::{IrqSourceRegisters, IrqSourceStatus, irq_offset};
use pdma_model_contract::semantic::{
    EpochIrqSource, IrqCause, IrqLine, IrqRoute, IrqSnapshot, IrqSource, IrqVector,
};

pub(super) async fn configure<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    source: EpochIrqSource,
    route: IrqRoute,
) -> PdmaPortResult<()> {
    let _window = port.queue_window.lock().await;
    let (kind, vector) = match route {
        IrqRoute::Line(line) => (0, line.vector()),
        IrqRoute::Message(vector) => (1, vector),
    };
    write_source(
        port,
        source,
        (IrqSourceRegisters::ROUTE_KIND.offset(), kind),
    )
    .await?;
    write_source(
        port,
        source,
        (
            IrqSourceRegisters::ROUTE_VECTOR.offset(),
            u64::try_from(vector.raw()).map_err(|_| PortError::SubmissionState)?,
        ),
    )
    .await
}

pub(super) async fn write_source<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    source: EpochIrqSource,
    value: (usize, u64),
) -> PdmaPortResult<()> {
    let address = source_address(port, source, value.0)?;
    registers::write(port, address, value.1).await
}

pub(super) async fn snapshot<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
) -> PdmaPortResult<Option<IrqSnapshot>> {
    let handles = port.with_state(|state| {
        state
            .queues
            .values()
            .map(|queue| queue.handle)
            .collect::<Vec<_>>()
    })?;
    if handles.is_empty() {
        let epoch_offset =
            u32::try_from(pdma_model_contract::abi::registers::GlobalRegisters::EPOCH.offset())
                .map_err(|_| PortError::SubmissionState)?;
        registers::read(port, epoch_offset).await?;
    }
    let mut pending = None;
    for event in [false, true] {
        for handle in &handles {
            let source = if event {
                handle.event_irq_source()
            } else {
                handle.completion_irq_source()
            };
            let status = registers::read(
                port,
                source_address(port, source, IrqSourceRegisters::STATUS.offset())?,
            )
            .await?;
            let status = IrqSourceStatus::try_from(
                u32::try_from(status).map_err(|_| PortError::SubmissionState)?,
            )
            .map_err(|_| PortError::SubmissionState)?;
            if !status.cause_pending() {
                continue;
            }
            if registers::read(
                port,
                source_address(port, source, IrqSourceRegisters::ROUTE_KIND.offset())?,
            )
            .await?
                != 0
            {
                continue;
            }
            let vector = registers::read(
                port,
                source_address(port, source, IrqSourceRegisters::ROUTE_VECTOR.offset())?,
            )
            .await?;
            let vector =
                IrqVector::new(usize::try_from(vector).map_err(|_| PortError::SubmissionState)?);
            let snapshot = IrqSnapshot::new(
                IrqCause::new(IrqLine::new(vector), source),
                status.line_asserted(),
            );
            if snapshot.asserted() {
                return Ok(Some(snapshot));
            }
            pending.get_or_insert(snapshot);
        }
    }
    Ok(pending)
}

fn source_address<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    source: EpochIrqSource,
    local: usize,
) -> PdmaPortResult<u32> {
    port.with_state(|state| {
        if source.epoch() != state.epoch {
            return Err(PortError::StaleEpoch {
                expected: state.epoch,
                actual: source.epoch(),
            });
        }
        let (index, source_index) = match source.source() {
            IrqSource::Completion(id) => (id.raw(), 0),
            IrqSource::Event(id) => (id.raw(), 1),
        };
        if !state.queues.values().any(|queue| {
            source == queue.handle.completion_irq_source()
                || source == queue.handle.event_irq_source()
        }) {
            return Err(PortError::UnknownIrqSource(source.source()));
        }
        let index = u32::try_from(index).map_err(|_| PortError::SubmissionState)?;
        irq_offset(index, source_index, local).map_err(|_| PortError::SubmissionState)
    })?
    .map_err(Into::into)
}
