//! Context-selector wire behavior.

use pdma_model_contract::dma::{Command, CompletionFlagsValues, Endpoint};

#[test]
fn command_preserves_independent_context_selectors_when_decoded_and_encoded() {
    // Given: distinct source, destination and SG selectors in a published command.
    let command = Command {
        copy_length: 32,
        sg_context: 0,
        id: 7,
        source: Endpoint::linear(0x1000, 0),
        destination: Endpoint::scatter_gather(0x2000, 2),
        flags: CompletionFlagsValues { ioc: true },
    };
    let mut bytes = command.to_bytes().expect("valid descriptor");
    bytes[12..16].copy_from_slice(&11_u32.to_le_bytes());
    bytes[52..56].copy_from_slice(&22_u32.to_le_bytes());
    bytes[100..104].copy_from_slice(&33_u32.to_le_bytes());

    // When: the hardware decodes the descriptor and republishes its representation.
    let decoded = Command::from_bytes(&bytes).expect("valid context selectors");

    // Then: none of the three selectors is lost or substituted with another.
    assert_eq!(decoded.source.context, 11);
    assert_eq!(decoded.destination.context, 22);
    assert_eq!(decoded.sg_context, 33);
    assert_eq!(decoded.to_bytes().expect("decoded descriptor"), bytes);
}
