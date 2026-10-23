use pdma_model_contract::hw_components::control::ControlConfiguration;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::memory::{ContextAttributesAddressSpace, FunctionId};
use pdma_model_hw_components::control::ControlChannel;

#[test]
fn enabled_control_freezes_attributes_and_keeps_manager_identity() {
    // Given: PF configures PASID zero before enabling its independent control path.
    let manager = FunctionId::new(3);
    let mut control = ControlChannel::new(0, manager);
    control
        .host_write(
            manager,
            ControlRegisters::CONFIG.offset() + ControlConfiguration::ATTRIBUTES.offset(),
            &1_u32.to_le_bytes(),
        )
        .unwrap();
    control
        .host_write(
            manager,
            ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset(),
            &1_u32.to_le_bytes(),
        )
        .unwrap();
    // When: a live attribute change is attempted.
    let changed = control.host_write(
        manager,
        ControlRegisters::CONFIG.offset() + ControlConfiguration::ATTRIBUTES.offset(),
        &0_u32.to_le_bytes(),
    );
    // Then: the fixed manager and PASID configuration survive the rejected write.
    assert!(changed.is_err());
    assert_eq!(control.memory_identity().unwrap().function, manager);
    assert_eq!(
        control.memory_identity().unwrap().attributes.address_space,
        ContextAttributesAddressSpace::Pasid
    );
}
