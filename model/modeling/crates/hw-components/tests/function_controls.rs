//! Trusted ingress identity and independent management doorbells.
use hardware_interface_schema::RegisterAccessError;
use pdma_model_contract::{ControlRegisters, FunctionControlRegisters, FunctionId};
use pdma_model_hw_components::{ControlError, FunctionControls};

#[test]
fn guessed_peer_addresses_cannot_publish_or_notify() {
    // Given: two control blocks and a Function-zero requester.
    let mut controls = FunctionControls::new(0x1000, 2).unwrap();
    let layout = FunctionControlRegisters::new(2).unwrap();
    let peer = 0x1000 + layout.functions().offset(1).unwrap();
    // When: it guesses the peer bootstrap and request doorbell addresses.
    assert_eq!(
        controls.host_write(FunctionId::new(0), peer, &77_u64.to_le_bytes()),
        Err(RegisterAccessError::Other(ControlError::Permission))
    );
    assert_eq!(
        controls.host_write(
            FunctionId::new(0),
            peer + ControlRegisters::REQUEST.offset(),
            &1_u32.to_le_bytes()
        ),
        Err(RegisterAccessError::Other(ControlError::Permission))
    );
    let mut bytes = [0xa5; 8];
    assert_eq!(
        controls.host_read(FunctionId::new(0), peer, &mut bytes),
        Err(RegisterAccessError::Other(ControlError::Permission))
    );
    // Then: the peer retains zero bootstrap and has no notification.
    assert_eq!(bytes, [0xa5; 8]);
    controls.firmware_read(peer, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 8]);
    controls.take_notifications(|_, request, response| assert!(!request && !response));
}

#[test]
fn pulses_keep_function_identity_and_coalesce_independently() {
    // Given: one pending request from each Function and repeated responses to Function one.
    let mut controls = FunctionControls::new(0, 2).unwrap();
    let layout = FunctionControlRegisters::new(2).unwrap();
    for index in 0..2 {
        let base = layout.functions().offset(index).unwrap();
        controls
            .host_write(
                FunctionId::new(u32::try_from(index).unwrap()),
                base + ControlRegisters::REQUEST.offset(),
                &1_u32.to_le_bytes(),
            )
            .unwrap();
    }
    for _ in 0..3 {
        controls
            .firmware_write(
                layout.functions().offset(1).unwrap() + ControlRegisters::RESPONSE.offset(),
                &1_u32.to_le_bytes(),
            )
            .unwrap();
    }
    // When: the platform transfers pulses into pending interrupt destinations.
    let mut seen = Vec::new();
    controls.take_notifications(|id, request, response| seen.push((id.raw(), request, response)));
    // Then: destination identity is preserved and every source is consumed once.
    assert_eq!(seen, [(0, true, false), (1, true, true)]);
    controls.take_notifications(|_, request, response| assert!(!request && !response));
}
