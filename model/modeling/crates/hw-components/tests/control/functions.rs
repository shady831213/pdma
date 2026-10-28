//! PF control-aperture permission tests.
use hw_model_schema::RegisterAccessError;
use pdma_model_contract::hw_components::control::ControlNotificationRegisters;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::control::{ControlChannel, ControlError};

#[test]
fn vf_cannot_publish_read_or_notify_through_pf_registers() {
    // Given: a platform-selected PF identity, unrelated to channel indices.
    let pf = FunctionId::new(7);
    let vf = FunctionId::new(0);
    let mut controls = ControlChannel::new(0x1000, pf);
    // When: a VF accesses every leaf, including all notification registers.
    for register in ControlRegisters::resolve().unwrap().registers() {
        let offset = register.offset();
        let mut bytes = vec![0; register.definition().width()];
        assert_eq!(
            controls.host_write(vf, 0x1000 + offset, &bytes),
            Err(RegisterAccessError::Other(ControlError::Permission))
        );
        assert_eq!(
            controls.host_read(vf, 0x1000 + offset, &mut bytes),
            Err(match register.definition().access() {
                hw_model_schema::Access::WriteOnly => RegisterAccessError::ReadForbidden { offset },
                hw_model_schema::Access::ReadOnly | hw_model_schema::Access::ReadWrite =>
                    RegisterAccessError::Other(ControlError::Permission),
            })
        );
    }
    let mut bytes = [0xa5; 8];
    assert_eq!(
        controls.host_read(vf, 0x1000, &mut bytes),
        Err(RegisterAccessError::Other(ControlError::Permission))
    );
    // Then: no state, read output or notification changes.
    assert_eq!(bytes, [0xa5; 8]);
    controls.firmware_read(0x1000, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 8]);
    assert!(!controls.take_request_notification());
    assert!(!controls.take_response_notification());
    assert!(!controls.take_event_notification());
    controls
        .host_write(pf, 0x1000, &77_u64.to_le_bytes())
        .unwrap();
    controls.firmware_read(0x1000, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 77);
}

#[test]
fn only_pf_and_firmware_can_ring_their_respective_direction() {
    // Given: a sole PF control channel with independent notification directions.
    let pf = FunctionId::new(2);
    let mut controls = ControlChannel::new(0, pf);
    // When: both authorized endpoints notify repeatedly.
    for _ in 0..3 {
        controls
            .host_write(pf, ControlRegisters::REQUEST.offset(), &1_u32.to_le_bytes())
            .unwrap();
        controls
            .firmware_write(
                ControlRegisters::NOTIFICATION.offset()
                    + ControlNotificationRegisters::RESPONSE.offset(),
                &1_u32.to_le_bytes(),
            )
            .unwrap();
    }
    // Then: each direction coalesces independently and is consumed once.
    assert!(controls.take_request_notification());
    assert!(!controls.take_request_notification());
    assert!(controls.take_response_notification());
    assert!(!controls.take_response_notification());
}
