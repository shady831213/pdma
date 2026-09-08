use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::queue::exact::PdmaControlError;
use crate::trace::TraceSink;
use hardware_interface_schema::RegisterLayout;
use hardware_interface_schema::register::RegisterSpec;
use pdma_model_contract::abi::registers::{
    IrqSourceRegisters, IrqSourceRegistersRegisterId, IrqSourceRegistersValues, IrqStateBit,
};
use pdma_model_contract::semantic::{
    CompletionRingId, IrqLine, IrqRoute, IrqSource, IrqVector, QueueId,
};

impl PdmaIrqController {
    pub(crate) fn read_direct(
        &self,
        index: u32,
        source_index: u32,
        offset: usize,
        queues: &QueueBlock,
    ) -> Option<u64> {
        let source = source(index, source_index, queues).ok()?;
        IrqSourceRegisters::read_register(&self.direct_values(source, queues), offset)
    }

    fn direct_values(&self, source: IrqSource, queues: &QueueBlock) -> IrqSourceRegistersValues {
        let observed = self.observe(source);
        let (kind, vector) = match observed.route {
            IrqRoute::Line(line) => (0, line.vector()),
            IrqRoute::Message(vector) => (1, vector),
        };
        let mut status = IrqStateBit::Present.mask();
        for (set, bit) in [
            (observed.enabled(), IrqStateBit::Enabled),
            (observed.masked(), IrqStateBit::Masked),
            (observed.cause_pending(), IrqStateBit::CausePending),
            (observed.deliverable(), IrqStateBit::Deliverable),
            (observed.line_asserted(), IrqStateBit::LineAsserted),
            (
                Self::records_pending(source, queues).unwrap_or(false),
                IrqStateBit::RecordsPending,
            ),
        ] {
            if set {
                status |= bit.mask();
            }
        }
        IrqSourceRegistersValues {
            route_kind: kind.into(),
            route_vector: u64::try_from(vector.raw()).unwrap_or(u64::MAX).into(),
            enabled: u32::from(observed.enabled()).into(),
            masked: u32::from(observed.masked()).into(),
            status: status.into(),
            pending_messages: u64::try_from(observed.pending_messages)
                .unwrap_or(u64::MAX)
                .into(),
            ..IrqSourceRegistersValues::default()
        }
    }

    pub(crate) fn write_direct(
        &mut self,
        indices: (u32, u32),
        register: RegisterSpec<IrqSourceRegistersRegisterId>,
        value: u64,
        queues: &QueueBlock,
        trace: &mut TraceSink,
    ) -> Result<bool, PdmaControlError> {
        use IrqSourceRegistersRegisterId as Id;
        let source = source(indices.0, indices.1, queues)?;
        let before = self.direct_values(source, queues);
        let mut values = before;
        IrqSourceRegisters::write_register(&mut values, register.offset(), value)
            .ok_or(PdmaControlError::InvalidTransaction)?;
        let was_asserted = self.line_asserted();
        match register.id() {
            Id::RouteKind | Id::RouteVector => {
                let vector = IrqVector::new(
                    usize::try_from(values.route_vector.raw())
                        .map_err(|_| PdmaControlError::InvalidTransaction)?,
                );
                let route = match values.route_kind.raw() {
                    0 => IrqRoute::Line(IrqLine::new(vector)),
                    1 => IrqRoute::Message(vector),
                    _ => return Err(PdmaControlError::InvalidTransaction),
                };
                self.configure_route(source, route)
                    .map_err(|_| PdmaControlError::InvalidTransaction)?;
            }
            Id::Enabled => match value {
                0 => self.disable(source),
                1 => self.enable(source),
                _ => return Err(PdmaControlError::InvalidTransaction),
            },
            Id::Masked => match value {
                0 => self.unmask(source),
                1 => self.mask(source),
                _ => return Err(PdmaControlError::InvalidTransaction),
            },
            Id::Ack => {
                if value != 1 {
                    return Err(PdmaControlError::InvalidTransaction);
                }
                self.ack(
                    source,
                    Self::records_pending(source, queues).unwrap_or(false),
                );
                trace.irq_device_ack(source);
            }
            Id::Status | Id::PendingMessages => return Err(PdmaControlError::InvalidTransaction),
        }
        self.trace_line_transition(was_asserted, trace);
        Ok(before != self.direct_values(source, queues))
    }
}

fn source(
    index: u32,
    source_index: u32,
    queues: &QueueBlock,
) -> Result<IrqSource, PdmaControlError> {
    if index >= queues.queue_count() {
        return Err(PdmaControlError::InvalidQueue);
    }
    let index = usize::try_from(index).map_err(|_| PdmaControlError::InvalidQueue)?;
    match source_index {
        0 => Ok(IrqSource::Completion(CompletionRingId::new(index))),
        1 => Ok(IrqSource::Event(QueueId::new(index))),
        _ => Err(PdmaControlError::InvalidQueue),
    }
}
