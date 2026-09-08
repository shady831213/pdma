use super::DeviceCore;
use hardware_interface_schema::RegisterLayout;
use pdma_model_contract::abi::mmio::{
    GlobalRegisters, IrqRegisters, PdmaMmioMap, PdmaMmioMapRegisterId, QueueRegisters, RegisterSpec,
};

impl DeviceCore {
    pub(crate) fn read_register(&self, register: RegisterSpec) -> Option<Vec<u8>> {
        let offset = register.offset();
        let value = match register.id() {
            PdmaMmioMapRegisterId::Global => GlobalRegisters::read_register(
                &self.global_registers,
                offset.checked_sub(PdmaMmioMap::GLOBAL.offset())?,
            ),
            PdmaMmioMapRegisterId::Queue => QueueRegisters::read_register(
                &self.queues.registers,
                offset.checked_sub(PdmaMmioMap::QUEUE.offset())?,
            ),
            PdmaMmioMapRegisterId::Irq => IrqRegisters::read_register(
                &self.irq.registers,
                offset.checked_sub(PdmaMmioMap::IRQ.offset())?,
            ),
        }?;
        value
            .to_le_bytes()
            .get(..register.width())
            .map(<[u8]>::to_vec)
    }

    pub(crate) fn write_mmio_register(
        &mut self,
        register: RegisterSpec,
        value: u64,
    ) -> Option<bool> {
        let offset = register.offset();
        let changed = match register.id() {
            PdmaMmioMapRegisterId::Global => {
                let local = offset.checked_sub(PdmaMmioMap::GLOBAL.offset())?;
                GlobalRegisters::write_register(&mut self.global_registers, local, value)?;
                local == GlobalRegisters::COMMAND.offset() && self.execute_global_command()
            }
            PdmaMmioMapRegisterId::Queue => {
                let local = offset.checked_sub(PdmaMmioMap::QUEUE.offset())?;
                QueueRegisters::write_register(&mut self.queues.registers, local, value)?;
                local == QueueRegisters::COMMAND.offset() && self.execute_queue_command()
            }
            PdmaMmioMapRegisterId::Irq => {
                let local = offset.checked_sub(PdmaMmioMap::IRQ.offset())?;
                IrqRegisters::write_register(&mut self.irq.registers, local, value)?;
                local == IrqRegisters::COMMAND.offset()
                    && self.irq.execute_mmio(&self.queues, &mut self.trace)
            }
        };
        Some(changed)
    }
}
