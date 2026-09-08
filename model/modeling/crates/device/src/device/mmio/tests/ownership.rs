use crate::DeviceCore;
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::mmio::{PdmaMmioMap, register_at};
use pdma_model_contract::semantic::{DeviceEpoch, IrqLine, IrqVector, ResourceConfig};
use std::sync::Arc;

#[test]
fn queue_and_irq_own_independent_register_bytes() {
    // Given: independently constructed hardware owners.
    let mut queues = QueueBlock::default();
    let mut irq = PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0)));
    queues.registers.id = 17.into();
    irq.registers.source_id = 29.into();
    // When: the queue owner resets without a device root or IRQ reset.
    queues.clear();
    // Then: only its own register bytes reset.
    assert_eq!(queues.registers.id.raw(), 0);
    assert_eq!(irq.registers.source_id.raw(), 29);
}

#[test]
fn every_owned_register_defaults_to_its_declared_reset_value()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Given: one default register set per hardware owner.
    let core = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    // When: the typed fields and raw endpoint expose those initial values.
    let typed = super::coherence::typed_values(&core);
    // Then: every canonical leaf has its declared reset value in both representations.
    for (register, value) in PdmaMmioMap::resolve()?.registers().iter().zip(typed) {
        let definition = register.definition();
        assert_eq!(value, definition.reset_value());
        assert_eq!(
            core.read_register(register_at(register.offset()).ok_or("register")?)
                .ok_or("register read")?,
            definition
                .reset_value()
                .to_le_bytes()
                .get(..definition.width())
                .ok_or("width")?
        );
    }
    Ok(())
}
