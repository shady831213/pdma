//! Timestamp wire acceptance independent of its generated constructor.
use pdma_model_contract::dma::Command;

#[test]
fn timestamp_decodes_and_preserves_target_and_context() {
    // Given: opcode 8 and a fixed 64-bit destination in context 7.
    let mut wire = [0_u8; 112];
    wire[0] = 39;
    wire[8] = 8;
    wire[16..24].copy_from_slice(&0x1234_5678_u64.to_le_bytes());
    wire[24..28].copy_from_slice(&7_u32.to_le_bytes());
    // When: the hardware wire decoder accepts the command.
    let command = Command::from_bytes(&wire).expect("Timestamp command must decode");
    // Then: all fields and reserved bytes survive a canonical round trip.
    assert_eq!(command.id, 39);
    assert_eq!(command.to_bytes().unwrap(), wire);
}
