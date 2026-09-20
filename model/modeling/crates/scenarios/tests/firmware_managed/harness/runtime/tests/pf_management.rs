use super::*;
use hardware_interface_schema::RegisterAccessError;
use pdma_model_contract::ControlRegisters;
use pdma_model_hw_components::ControlError;

#[tokio::test]
async fn vf_cannot_enable_a_firmware_control_channel() {
    // Given: an undriven platform with PF and VF work ports.
    let (_system, mut hosts) = Managed::with_functions(
        SharedMemoryAccess::from(Arc::new(SharedMemory::new())),
        1,
        2,
    )
    .unwrap();
    let vf = hosts.pop().unwrap();
    let pf = hosts.pop().unwrap();
    // When: VF and PF attempt to enable the management channel through their own ports.
    let denied = vf
        .bus
        .write(
            vf.bus.addresses().control + u32::try_from(ControlRegisters::ENABLE.offset()).unwrap(),
            &1_u32.to_le_bytes(),
        )
        .await;
    pf.bus
        .write(
            pf.bus.addresses().control + u32::try_from(ControlRegisters::ENABLE.offset()).unwrap(),
            &1_u32.to_le_bytes(),
        )
        .await
        .unwrap();
    // Then: only PF can publish control-channel configuration.
    assert_eq!(
        denied,
        Err(AccessError::Control(RegisterAccessError::Other(
            ControlError::Permission
        )))
    );
}

#[tokio::test]
async fn vf_cannot_read_control_bootstrap() {
    // Given: PF has published an address into its control bootstrap.
    let (_system, mut hosts) = Managed::with_functions(
        SharedMemoryAccess::from(Arc::new(SharedMemory::new())),
        1,
        2,
    )
    .unwrap();
    let vf = hosts.pop().unwrap();
    let pf = hosts.pop().unwrap();
    pf.bus
        .write(pf.bus.addresses().control, &0x1234_u64.to_le_bytes())
        .await
        .unwrap();
    let mut bytes = [0xa5; 8];
    // When: VF attempts to read the control bootstrap exposed in its address space.
    let result = vf.bus.read(vf.bus.addresses().control, &mut bytes).await;
    // Then: permission rejection preserves its output buffer.
    assert_eq!(
        result,
        Err(AccessError::Control(RegisterAccessError::Other(
            ControlError::Permission
        )))
    );
    assert_eq!(bytes, [0xa5; 8]);
}
