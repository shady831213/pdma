use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::mmio::{
    IrqCommand, IrqSourceStatus, IrqStateBit, MmioResult, MmioResultDetails,
};
use pdma_model_contract::semantic::irq::{IrqLine, IrqRoute, IrqSource, IrqVector};
use pdma_model_contract::semantic::{CompletionRingId, DeviceEpoch, EpochIrqSource, QueueId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IrqSourceView {
    pub(crate) source: EpochIrqSource,
    pub(crate) route: IrqRoute,
    pub(crate) status: IrqSourceStatus,
    pub(crate) pending_messages: usize,
}

pub(crate) fn observe_irq_source(
    irq: &PdmaIrqController,
    queues: &QueueBlock,
    source: EpochIrqSource,
) -> ModelResult<IrqSourceView> {
    let (observed, records_pending) = irq.observe_source(source, queues)?;
    let mut bits = IrqStateBit::Present.mask();
    for (set, bit) in [
        (observed.enabled(), IrqStateBit::Enabled),
        (observed.masked(), IrqStateBit::Masked),
        (observed.cause_pending(), IrqStateBit::CausePending),
        (observed.deliverable(), IrqStateBit::Deliverable),
        (observed.line_asserted(), IrqStateBit::LineAsserted),
        (records_pending, IrqStateBit::RecordsPending),
    ] {
        if set {
            bits |= bit.mask();
        }
    }
    let status = IrqSourceStatus::try_from(bits).map_err(|_| ModelError::DmaSubmissionState)?;
    Ok(IrqSourceView {
        source,
        route: observed.route,
        status,
        pending_messages: observed.pending_messages,
    })
}

pub(crate) fn decode_source(epoch: u64, kind: u32, id: u64) -> Option<EpochIrqSource> {
    let raw = usize::try_from(id).ok()?;
    let source = match kind {
        0 => IrqSource::Completion(CompletionRingId::new(raw)),
        1 => IrqSource::Event(QueueId::new(raw)),
        _ => return None,
    };
    Some(EpochIrqSource::new(DeviceEpoch::new(epoch), source))
}

pub(crate) fn decode_route(kind: u32, vector: u64) -> Option<IrqRoute> {
    let vector = IrqVector::new(usize::try_from(vector).ok()?);
    match kind {
        0 => Some(IrqRoute::Line(IrqLine::new(vector))),
        1 => Some(IrqRoute::Message(vector)),
        _ => None,
    }
}

pub(crate) fn encode_source(source: IrqSource) -> (u32, u64) {
    match source {
        IrqSource::Completion(id) => (0, usize_to_u64(id.raw())),
        IrqSource::Event(id) => (1, usize_to_u64(id.raw())),
    }
}

pub(crate) fn encode_route(route: IrqRoute) -> (u32, u64) {
    match route {
        IrqRoute::Line(line) => (0, usize_to_u64(line.vector().raw())),
        IrqRoute::Message(vector) => (1, usize_to_u64(vector.raw())),
    }
}

pub(crate) fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).map_or(u64::MAX, core::convert::identity)
}

pub(crate) fn map_mmio_error(
    error: ModelError,
    command: IrqCommand,
    source: EpochIrqSource,
) -> MmioResultDetails {
    let (kind, id) = encode_source(source.source());
    let (result, details) = match error {
        ModelError::StaleDeviceEpoch { expected, actual } => {
            (MmioResult::StaleEpoch, [expected.raw(), actual.raw(), 0])
        }
        ModelError::UnknownIrqSource(_) => (MmioResult::InvalidIrqSource, [u64::from(kind), id, 0]),
        ModelError::IrqSourceActive(_) => (MmioResult::IrqSourceActive, [u64::from(kind), id, 0]),
        _ => (
            MmioResult::InvalidTransaction,
            [(2_u64 << 32) | u64::from(command.code()), 0, 0],
        ),
    };
    MmioResultDetails { result, details }
}
