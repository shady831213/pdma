//! Recoverable faults suspend work instead of terminating it on the response ring.

use pdma_model_contract::dma::Response;

#[test]
fn retired_page_fault_status_is_rejected() {
    // Given: the former page-fault completion encoding.
    let mut bytes = [0_u8; Response::RECORD_SIZE];
    bytes[8..12].copy_from_slice(&6_u32.to_le_bytes());
    // When: a software consumer decodes the device completion.
    let decoded = Response::from_bytes(&bytes);
    // Then: a fault cannot masquerade as a terminal completion.
    assert!(
        decoded.is_err(),
        "obsolete page-fault completion accepted: {decoded:?}"
    );
}
