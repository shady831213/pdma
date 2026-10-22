#![doc = "Copy-engine wire and notification boundary assertions."]

use hardware_interface_schema::Access;
use pdma_model_contract::dma::{
    Capabilities, ChannelIrqCauseValues, ChannelIrqMaskValues, DmaDiscovery, TrapAck, TrapAckValues,
};

#[test]
fn trap_ack_has_write_one_storage_and_write_only_access() {
    // Given: an acknowledgment request with reserved bits clear.
    let request = TrapAckValues::default().with_request(true).unwrap();
    // When: the request is encoded for MMIO.
    let bytes = request.raw().to_le_bytes();
    // Then: exactly bit zero is sent through a write-only register.
    assert_eq!(bytes, [1, 0, 0, 0]);
    assert_eq!(TrapAck::SPEC.access(), Access::WriteOnly);
    assert_eq!(TrapAck::SPEC.writable_mask(), 1);
}

#[test]
fn causes_and_masks_keep_error_independent_of_optional_notifications() {
    // Given: distinct event, Trap, stopping error and fatal causes.
    let cause = ChannelIrqCauseValues::default()
        .with_event_non_empty(true)
        .unwrap()
        .with_trap(true)
        .unwrap()
        .with_error(true)
        .unwrap()
        .with_channel_fatal(true)
        .unwrap();
    // When: both optional notification sources are masked.
    let mask = ChannelIrqMaskValues::default()
        .with_event_non_empty(true)
        .unwrap()
        .with_trap(true)
        .unwrap();
    // Then: error and fatal have no mask bits.
    assert_eq!(cause.raw(), 15);
    assert_eq!(mask.raw(), 3);
}

#[test]
fn discovered_atomic_capabilities_are_read_only() {
    // Given: discovery metadata for atomic support.
    let map = DmaDiscovery::resolve().unwrap();
    // When: software resolves the capability register.
    let register = map
        .registers()
        .iter()
        .find(|register| register.offset() == 4)
        .unwrap();
    // Then: capability support cannot be programmed through MMIO.
    assert_eq!(register.definition().access(), Access::ReadOnly);
    assert_eq!(register.definition().writable_mask(), 0);
    assert_eq!(Capabilities::SPEC.width(), 4);
}
