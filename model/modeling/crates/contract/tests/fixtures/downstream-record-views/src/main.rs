use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadValues, AbiCommandView, AbiCompletionView, AbiEventView,
    IndirectDestinationEntrySchemaView,
};

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
    let descriptor = AbiCommandView::new(&DESCRIPTOR)?;
    let header = descriptor.header()?;
    let values = AbiCommand::from_bytes(&DESCRIPTOR)?;
    let completion = AbiCompletionView::new(&COMPLETION)?;
    let event = AbiEventView::new(&EVENT)?;
    let indirect = IndirectDestinationEntrySchemaView::new(&INDIRECT)?;
    let AbiCommandPayloadValues::Copy(copy) = values.payload
    else {
        return Err(std::io::Error::other("copy fixture decoded as another variant").into());
    };
    let exact = AbiCommand::decode(&DESCRIPTOR)?;
    let mut encoded = [0_u8; 64];
    exact.encode(&mut encoded)?;
    let roundtrip = encoded == DESCRIPTOR
        && event.version()? == header.version()?
        && values.header.version == header.version()?
        && completion.command_id()? == values.header.command_id
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
