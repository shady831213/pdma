use crate::support::{ADDRESS, Spaces};
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use pdma_model_contract::hw_components::control::ControlConfiguration;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::control::ControlChannel;

#[test]
fn control_memory_uses_fixed_manager_and_independent_pasid_without_table() {
    // Given: PF zero configures PASID zero before bootstrap, with no context table.
    let spaces = Spaces::new().unwrap();
    let mut control = ControlChannel::new(0, FunctionId::new(0));
    control
        .host_write(
            FunctionId::new(0),
            ControlRegisters::CONFIG.offset() + ControlConfiguration::ATTRIBUTES.offset(),
            &1_u32.to_le_bytes(),
        )
        .unwrap();
    control
        .host_write(
            FunctionId::new(0),
            ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset(),
            &1_u32.to_le_bytes(),
        )
        .unwrap();
    // When: firmware accesses Host memory through the borrowed control port.
    let bytes = control
        .memory(&spaces)
        .read_buffer::<[u8; 4]>(ADDRESS, MemoryAlignment::ONE)
        .unwrap();
    // Then: PASID-zero bytes are selected without any table allocation.
    assert_eq!(bytes, [22; 4]);
}
