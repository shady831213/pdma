use super::DeviceCore;
use crate::queue::exact::PdmaControlError;
use hardware_interface_schema::RegisterLayout;
use pdma_model_contract::abi::registers::{GlobalRegisters, RegisterLocation};

impl DeviceCore {
    pub(crate) fn read_register(&self, location: RegisterLocation) -> Option<Vec<u8>> {
        let (value, width) = match location {
            RegisterLocation::Global { register } => (
                GlobalRegisters::read_register(&self.global_values(), register.offset())?,
                register.width(),
            ),
            RegisterLocation::Queue { index, register } => (
                self.queues.read_direct(index, register.offset())?,
                register.width(),
            ),
            RegisterLocation::Irq {
                index,
                source_index,
                register,
            } => (
                self.queues
                    .read_irq(index, source_index, register.offset())?,
                register.width(),
            ),
        };
        value.to_le_bytes().get(..width).map(<[u8]>::to_vec)
    }

    pub(crate) fn write_mmio_register(
        &mut self,
        location: RegisterLocation,
        value: u64,
    ) -> Result<bool, PdmaControlError> {
        match location {
            RegisterLocation::Global { register } => self.write_global(register.offset(), value),
            RegisterLocation::Queue { index, register } => {
                self.write_queue_register(index, register, value)
            }
            RegisterLocation::Irq {
                index,
                source_index,
                register,
            } => self
                .queues
                .write_irq((index, source_index), register, value, &mut self.trace),
        }
    }
}
