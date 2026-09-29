//! Recoverable completion is a distinct wire result.

use pdma_model_contract::dma::Response;

#[test]
fn recoverable_status_is_accepted_without_reusing_existing_error_codes() {
    // Given: the next assigned status following existing success/error outcomes.
    let mut bytes = [0_u8; Response::RECORD_SIZE];
    bytes[8..12].copy_from_slice(&6_u32.to_le_bytes());
    bytes[32] = 1;
    // When: a software consumer decodes the device completion.
    let decoded = Response::from_bytes(&bytes);
    // Then: it can distinguish the new completion rather than rejecting its tag.
    assert!(
        decoded.is_ok(),
        "recoverable completion rejected: {decoded:?}"
    );
}
