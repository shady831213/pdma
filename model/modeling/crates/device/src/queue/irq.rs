use super::{PdmaQueue, QueueBlock};
use crate::irq::source::IrqSourceState;
use crate::queue::exact::PdmaControlError;
use crate::trace::TraceSink;
use hardware_interface_schema::register::RegisterSpec;
use pdma_model_contract::abi::registers::IrqSourceRegistersRegisterId;
use pdma_model_contract::semantic::{CompletionRingId, IrqSource, QueueId};

impl QueueBlock {
    pub(crate) fn irq_source(&self, source: IrqSource) -> Option<&IrqSourceState> {
        match source {
            IrqSource::Completion(ring) => self
                .contexts
                .get(ring.raw())
                .map(|queue| &queue.completion_irq),
            IrqSource::Event(id) => self.contexts.get(id.raw()).map(|queue| &queue.event_irq),
        }
    }

    pub(crate) fn irq_sources(&self) -> impl Iterator<Item = (IrqSource, &IrqSourceState)> {
        self.contexts
            .iter()
            .enumerate()
            .map(|(index, queue)| {
                (
                    IrqSource::Completion(CompletionRingId::new(index)),
                    &queue.completion_irq,
                )
            })
            .chain(
                self.contexts.iter().enumerate().map(|(index, queue)| {
                    (IrqSource::Event(QueueId::new(index)), &queue.event_irq)
                }),
            )
    }

    pub(crate) fn irq_source_mut(&mut self, source: IrqSource) -> Option<&mut IrqSourceState> {
        match source {
            IrqSource::Completion(ring) => self
                .contexts
                .get_mut(ring.raw())
                .map(|queue| &mut queue.completion_irq),
            IrqSource::Event(id) => self
                .contexts
                .get_mut(id.raw())
                .map(|queue| &mut queue.event_irq),
        }
    }

    pub(crate) fn read_irq(&self, index: u32, source_index: u32, offset: usize) -> Option<u64> {
        let index = usize::try_from(index).ok()?;
        let queue = self.contexts.get(index)?;
        let (source, completion) = match source_index {
            0 => (&queue.completion_irq, true),
            1 => (&queue.event_irq, false),
            _ => return None,
        };
        source.read_register(offset, queue.irq_records_pending(completion))
    }

    pub(crate) fn write_irq(
        &mut self,
        indices: (u32, u32),
        register: RegisterSpec<IrqSourceRegistersRegisterId>,
        value: u64,
        trace: &mut TraceSink,
    ) -> Result<bool, PdmaControlError> {
        let index = usize::try_from(indices.0).map_err(|_| PdmaControlError::InvalidQueue)?;
        let queue = self
            .contexts
            .get_mut(index)
            .ok_or(PdmaControlError::InvalidQueue)?;
        let records_pending = match indices.1 {
            0 => queue.irq_records_pending(true),
            1 => queue.irq_records_pending(false),
            _ => return Err(PdmaControlError::InvalidQueue),
        };
        let (state, source) = match indices.1 {
            0 => (
                &mut queue.completion_irq,
                IrqSource::Completion(CompletionRingId::new(index)),
            ),
            1 => (&mut queue.event_irq, IrqSource::Event(QueueId::new(index))),
            _ => return Err(PdmaControlError::InvalidQueue),
        };
        let changed = state.write_register(register, value, records_pending)?;
        if register.id() == IrqSourceRegistersRegisterId::Ack {
            trace.irq_device_ack(source);
        }
        Ok(changed)
    }
}

impl PdmaQueue {
    fn irq_records_pending(&self, completion: bool) -> bool {
        self.io
            .as_ref()
            .is_some_and(|io| io.records_pending(completion))
    }
}
