//! Independent contexts in complete Copy records.
use pdma_model_contract::dma::{Command, CopyCommand};
#[test]
fn command_preserves_independent_context_selectors_when_decoded_and_encoded() {
    let mut bytes = [0_u8; 96];
    bytes[4..8].copy_from_slice(&33_u32.to_le_bytes());
    bytes[24..28].copy_from_slice(&11_u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&22_u32.to_le_bytes());
    let decoded = Command::from_bytes(&bytes).unwrap();
    let Command::Copy(CopyCommand {
        source_context,
        destination_context,
        sg_context,
        ..
    }) = decoded
    else {
        panic!("Copy")
    };
    assert_eq!(
        (source_context, destination_context, sg_context),
        (11, 22, 33)
    );
    assert_eq!(decoded.to_bytes().unwrap(), bytes);
}
