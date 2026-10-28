use hw_model_schema::RegisterAccessError;
use pdma_model_contract::hw_components::control::ControlNotificationRegisters;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::memory::FunctionId;
use pdma_model_hw_components::control::{ControlChannel, ControlError};

#[test]
fn firmware_notifications_are_independent_and_do_not_replay() {
    // Given: a relocated PF aperture, with both firmware notifications pending.
    let base = 0x100;
    let pf = FunctionId::new(7);
    let mut controls = ControlChannel::new(base, pf);
    let response = base
        + ControlRegisters::NOTIFICATION.offset()
        + ControlNotificationRegisters::RESPONSE.offset();
    let event = base
        + ControlRegisters::NOTIFICATION.offset()
        + ControlNotificationRegisters::EVENT.offset();
    for address in [response, event, event] {
        controls
            .firmware_write(address, &1_u32.to_le_bytes())
            .unwrap();
    }
    // When: response is claimed, and an unrelated error report is updated.
    assert!(controls.take_response_notification());
    controls
        .firmware_write(
            base + ControlRegisters::NOTIFICATION.offset()
                + ControlNotificationRegisters::EVENT_ERROR.offset(),
            &2_u32.to_le_bytes(),
        )
        .unwrap();
    // Then: only the unclaimed event remains, coalesced exactly once.
    assert!(!controls.take_response_notification());
    assert!(controls.take_event_notification());
    assert!(!controls.take_event_notification());
    assert!(!controls.take_request_notification());
}

#[test]
fn event_doorbell_rejects_wrong_owner_and_malformed_access_without_notification() {
    // Given: an idle event notification source.
    let pf = FunctionId::new(7);
    let mut controls = ControlChannel::new(0, pf);
    let event =
        ControlRegisters::NOTIFICATION.offset() + ControlNotificationRegisters::EVENT.offset();
    // When: PF writes firmware's doorbell, or firmware supplies an invalid transaction.
    assert_eq!(
        controls.host_write(pf, event, &1_u32.to_le_bytes()),
        Err(RegisterAccessError::Other(ControlError::Permission))
    );
    for bytes in [&[3, 0, 0, 0][..], &[1, 0, 0], &[1, 0, 0, 0, 0, 0, 0, 0]] {
        assert!(controls.firmware_write(event, bytes).is_err());
    }
    // Then: rejected writes cannot produce any notification.
    assert!(!controls.take_request_notification());
    assert!(!controls.take_response_notification());
    assert!(!controls.take_event_notification());
}

#[test]
fn event_error_is_firmware_published_and_pf_readable() {
    // Given: a PF-owned management aperture.
    let pf = FunctionId::new(7);
    let mut controls = ControlChannel::new(0, pf);
    // When: firmware reports loss independently of management responses.
    controls.firmware_write(32, &2_u32.to_le_bytes()).unwrap();
    // Then: PF reads the error but cannot clear it, and VF cannot read it.
    let mut bytes = [0; 4];
    controls.host_read(pf, 32, &mut bytes).unwrap();
    assert_eq!(u32::from_le_bytes(bytes), 2);
    assert_eq!(
        controls.host_write(pf, 32, &0_u32.to_le_bytes()),
        Err(RegisterAccessError::Other(ControlError::Permission))
    );
    assert_eq!(
        controls.host_read(FunctionId::new(0), 32, &mut bytes),
        Err(RegisterAccessError::Other(ControlError::Permission))
    );
    assert!(!controls.take_response_notification());
}

#[test]
fn zero_notification_writes_preserve_pending_pulses_and_event_error() {
    // Given: retained event failure and either idle or pending firmware notifications.
    for pending in [false, true] {
        let pf = FunctionId::new(7);
        let mut controls = ControlChannel::new(0, pf);
        controls.firmware_write(32, &2_u32.to_le_bytes()).unwrap();
        for offset in [24, 28] {
            controls
                .firmware_write(offset, &u32::from(pending).to_le_bytes())
                .unwrap();
        }
        // When: firmware writes zero to both notification doorbells.
        for offset in [24, 28] {
            controls.firmware_write(offset, &[0; 4]).unwrap();
        }
        // Then: zero neither creates nor clears a pulse, and notification writes retain status.
        assert_eq!(controls.take_response_notification(), pending);
        assert_eq!(controls.take_event_notification(), pending);
        let mut bytes = [0; 4];
        controls.host_read(pf, 32, &mut bytes).unwrap();
        assert_eq!(u32::from_le_bytes(bytes), 2);
    }
}
