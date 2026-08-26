use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::abi::{AbiCommand, AbiEncodeError};
use pdma_model_contract::translation::{
    CopyBufferContext, CopyRuntimeAddresses, TranslationError, decode_copy_command,
    encode_copy_command,
};
use pdma_model_contract::semantic::{BufferId, CommandId, CompletionRingId, CopyCommand, CopyRequest, QueueId};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let command = CopyCommand::new(
        CommandId::new(7),
        QueueId::new(3),
        CompletionRingId::new(5),
        CopyRequest::new(BufferId::new(11), BufferId::new(12), 64),
    );
    let addresses =
        CopyRuntimeAddresses::new(MemoryAddress::new(0x1000), MemoryAddress::new(0x2000));
    let context = CopyBufferContext::from_command(command);
    let exact = encode_copy_command(command, addresses)?;
    let mut bytes = [0_u8; 64];
    exact.encode(&mut bytes)?;
    let decoded = AbiCommand::decode(&bytes)?;
    let (round_trip, decoded_addresses) = decode_copy_command(decoded, context)?;
    let range = TranslationError::from(AbiEncodeError::UnrepresentableId {
        value: u128::from(u64::MAX) + 1,
    });
    let source = source_label(&range)?;
    println!(
        "descriptor={};roundtrip={};address={:#x};source={source}",
        bytes.len(),
        round_trip == command && decoded_addresses == addresses,
        decoded_addresses.source().raw(),
    );
    Ok(())
}

fn source_label(error: &TranslationError) -> Result<&'static str, Box<dyn Error>> {
    match error
        .source()
        .and_then(|source| source.downcast_ref::<AbiEncodeError>())
    {
        Some(AbiEncodeError::UnrepresentableId { .. })
        | Some(AbiEncodeError::UnrepresentableModelId { .. }) => Ok("abi-id-range"),
        _ => Err("unexpected translation source chain".into()),
    }
}
