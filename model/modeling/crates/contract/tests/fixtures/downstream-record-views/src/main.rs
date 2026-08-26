use hardware_interface_runtime::MemoryAddress;
use pdma_model_contract::abi::{
    AbiCommand, CompletionSchemaView, DescriptorSchemaPayloadValues, DescriptorSchemaValues,
    DescriptorSchemaView, EventSchemaView, IndirectDestinationEntrySchemaView,
};
use pdma_model_contract::translation::{
    CopyBufferContext, CopyRuntimeAddresses, decode_copy_command, encode_copy_command,
};
use pdma_model_contract::semantic::{BufferId, CommandId, CompletionRingId, CopyCommand, CopyRequest, QueueId};

const DESCRIPTOR: [u8; 64] = [
    1, 0, 1, 0, 0, 0, 64, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 32,
    0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const COMPLETION: [u8; 32] = [
    1, 0, 32, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0,
];
const EVENT: [u8; 64] = [
    1, 0, 64, 0, 1, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 16,
    0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 1, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const INDIRECT: [u8; 8] = [0, 32, 0, 0, 0, 0, 0, 0];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor = DescriptorSchemaView::new(&DESCRIPTOR)?;
    let header = descriptor.header()?;
    let values = DescriptorSchemaValues::from_bytes(&DESCRIPTOR)?;
    let completion = CompletionSchemaView::new(&COMPLETION)?;
    let event = EventSchemaView::new(&EVENT)?;
    let indirect = IndirectDestinationEntrySchemaView::new(&INDIRECT)?;
    let DescriptorSchemaPayloadValues::Copy(copy) = values.payload
    else {
        return Err(std::io::Error::other("copy fixture decoded as another variant").into());
    };
    let semantic = CopyCommand::new(
        CommandId::new(usize::try_from(values.header.command_id)?),
        QueueId::new(usize::try_from(completion.origin_queue_id()?)?),
        CompletionRingId::new(5),
        CopyRequest::new(
            BufferId::new(11),
            BufferId::new(12),
            usize::try_from(copy.length)?,
        ),
    );
    let context = CopyBufferContext::from_command(semantic);
    let addresses = CopyRuntimeAddresses::new(
        MemoryAddress::new(copy.source_dma_address),
        MemoryAddress::new(copy.destination_dma_address),
    );
    let exact = encode_copy_command(semantic, addresses)?;
    let mut encoded = [0_u8; 64];
    exact.encode(&mut encoded)?;
    let decoded = AbiCommand::decode(&encoded)?;
    let (roundtrip, decoded_addresses) = decode_copy_command(decoded, context)?;
    let roundtrip = roundtrip == semantic
        && decoded_addresses == addresses
        && encoded == DESCRIPTOR
        && event.version()? == header.version()?
        && values.header.version == header.version()?
        && indirect.destination_dma_address()? == copy.destination_dma_address;
    println!(
        "descriptor={};completion={};event={};indirect={};roundtrip={roundtrip}",
        DESCRIPTOR.len(),
        COMPLETION.len(),
        EVENT.len(),
        INDIRECT.len()
    );
    Ok(())
}
