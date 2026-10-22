//! Recoverable faults suspend work instead of terminating it on the response ring.

use pdma_model_contract::dma::Event;

#[test]
fn success_code_is_rejected() {
    // Given: the a success encoding, which is not an error event.
    let mut bytes = [0_u8; Event::RECORD_SIZE];
    bytes[8..12].copy_from_slice(&0_u32.to_le_bytes());
    // When: a software consumer decodes the device completion.
    let decoded = Event::from_bytes(&bytes);
    // Then: a fault cannot masquerade as a terminal completion.
    assert!(decoded.is_err(), "success event accepted: {decoded:?}");
}
