use super::QueueBlock;
use super::exact::PdmaControlError;
use hardware_interface_schema::RegisterLayout;
use pdma_model_contract::abi::registers::{QueueRegisters, QueueRegistersValues, queue_offset};

impl QueueBlock {
    pub(crate) fn queue_count(&self) -> u32 {
        u32::try_from(self.contexts.len()).unwrap_or(u32::MAX)
    }

    pub(crate) fn is_active(&self, index: u32) -> bool {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.contexts.get(index))
            .is_some_and(|context| context.io.is_some())
    }

    pub(crate) fn configuration(
        &self,
        index: u32,
    ) -> Result<QueueRegistersValues, PdmaControlError> {
        let index = usize::try_from(index).map_err(|_| PdmaControlError::InvalidQueue)?;
        self.contexts
            .get(index)
            .map(|context| context.registers)
            .ok_or(PdmaControlError::InvalidQueue)
    }

    pub(crate) fn write_configuration(
        &mut self,
        index: u32,
        offset: usize,
        value: u64,
    ) -> Result<bool, PdmaControlError> {
        if ![
            QueueRegisters::SQ_BASE.offset(),
            QueueRegisters::SQ_DEPTH.offset(),
            QueueRegisters::CQ_BASE.offset(),
            QueueRegisters::CQ_DEPTH.offset(),
            QueueRegisters::EQ_BASE.offset(),
            QueueRegisters::EQ_DEPTH.offset(),
        ]
        .contains(&offset)
        {
            return Err(PdmaControlError::InvalidTransaction);
        }
        queue_offset(index, offset).map_err(|_| PdmaControlError::InvalidQueue)?;
        let index = usize::try_from(index).map_err(|_| PdmaControlError::InvalidQueue)?;
        let allocated = index == self.contexts.len();
        if allocated {
            let mut registers = QueueRegistersValues::default();
            QueueRegisters::write_register(&mut registers, offset, value)
                .ok_or(PdmaControlError::InvalidTransaction)?;
            self.contexts
                .try_reserve_exact(1)
                .map_err(|_| PdmaControlError::InvalidTransaction)?;
            self.contexts.push(super::PdmaQueue {
                registers,
                io: None,
            });
            return Ok(true);
        }
        let context = self
            .contexts
            .get_mut(index)
            .ok_or(PdmaControlError::InvalidQueue)?;
        if context.io.is_some() {
            return Err(PdmaControlError::InvalidTransaction);
        }
        let previous = QueueRegisters::read_register(&context.registers, offset);
        QueueRegisters::write_register(&mut context.registers, offset, value)
            .ok_or(PdmaControlError::InvalidTransaction)?;
        Ok(previous != Some(value))
    }

    pub(crate) fn read_direct(&self, index: u32, offset: usize) -> Option<u64> {
        queue_offset(index, offset).ok()?;
        let index = usize::try_from(index).ok()?;
        if index == self.contexts.len() {
            return QueueRegisters::read_register(&QueueRegistersValues::default(), offset);
        }
        let context = self.contexts.get(index)?;
        let mut registers = context.registers;
        if let Ok(active) = self.exact_ring(pdma_model_contract::semantic::QueueId::new(index)) {
            let submission = active.submission.ring();
            registers.sq_producer = submission.producer().raw().into();
            registers.sq_consumer = submission.consumer().raw().into();
            registers.sq_captured = active.submission.captured_producer().raw().into();
            registers.cq_producer = active.completion.producer().raw().into();
            registers.cq_consumer = active.completion.consumer().raw().into();
            registers.eq_producer = active.event.producer().raw().into();
            registers.eq_consumer = active.event.consumer().raw().into();
        }
        QueueRegisters::read_register(&registers, offset)
    }
}
