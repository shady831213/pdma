//! Context-selector wire behavior.

use pdma_model_contract::dma::{Command, CopyCommand, Endpoint};

#[test]
fn command_preserves_independent_context_selectors_when_decoded_and_encoded() {
    // Given: distinct source, destination and SG selectors in a published command.
    let command = Command::copy(
        7,
        CopyCommand {
            copy_length: 32,
            sg_context: 0,
            source: Endpoint::linear(0x1000, 0),
            destination: Endpoint::scatter_gather(0x2000, 2),
        },
    );
    let mut bytes = command.to_bytes().expect("valid descriptor");
    bytes[20..24].copy_from_slice(&11_u32.to_le_bytes());
    bytes[60..64].copy_from_slice(&22_u32.to_le_bytes());
    bytes[104..108].copy_from_slice(&33_u32.to_le_bytes());

    // When: the hardware decodes the descriptor and republishes its representation.
    let decoded = Command::from_bytes(&bytes).expect("valid context selectors");

    // Then: none of the three selectors is lost or substituted with another.
    let pdma_model_contract::dma::CommandPayloadValues::Copy(copy) = decoded.payload else {
        panic!("expected copy")
    };
    assert_eq!(copy.source.context, 11);
    assert_eq!(copy.destination.context, 22);
    assert_eq!(copy.sg_context, 33);
    assert_eq!(decoded.to_bytes().expect("decoded descriptor"), bytes);
}
