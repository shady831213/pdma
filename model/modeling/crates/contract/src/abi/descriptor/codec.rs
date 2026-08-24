use crate::abi::{
    ABI_VERSION, AbiEncodeError, AbiId, BROADCAST_COPY_OPCODE, COPY_OPCODE, DESCRIPTOR_FLAGS_MASK,
    DESCRIPTOR_SIZE, DescriptorSchemaValues, INDIRECT_DESTINATION_ENTRY_SIZE,
    IndirectDestinationEntrySchemaValues, STRIDED_COPY_OPCODE, VECTOR_ADD_OPCODE,
    codec::encode_schema_error,
};

use super::{AbiCommand, BroadcastDestinationEntry};

pub(super) fn encode_descriptor(
    command: AbiCommand,
    output: &mut [u8],
) -> Result<(), AbiEncodeError> {
    let required = usize::from(DESCRIPTOR_SIZE);
    let actual = output.len();
    let destination = output
        .get_mut(..required)
        .ok_or(AbiEncodeError::BufferTooShort { required, actual })?;
    let (command_id, opcode, words) = command_words(command);
    let encoded = DescriptorSchemaValues {
        version: ABI_VERSION,
        opcode,
        flags: DESCRIPTOR_FLAGS_MASK,
        size: DESCRIPTOR_SIZE,
        command_id: command_id.raw(),
        word0: words[0],
        word1: words[1],
        word2: words[2],
        word3: words[3],
        word4: words[4],
        word5: words[5],
    }
    .to_bytes()
    .map_err(encode_schema_error)?;
    destination.copy_from_slice(&encoded);
    Ok(())
}

pub(super) fn encode_destination(
    entry: BroadcastDestinationEntry,
    bytes: &mut [u8],
) -> Result<(), AbiEncodeError> {
    let required = usize::from(INDIRECT_DESTINATION_ENTRY_SIZE);
    let actual = bytes.len();
    let encoded = IndirectDestinationEntrySchemaValues {
        destination_iova: entry.address().raw(),
    }
    .to_bytes()
    .map_err(encode_schema_error)?;
    let destination = bytes
        .get_mut(..required)
        .ok_or(AbiEncodeError::BufferTooShort { required, actual })?;
    destination.copy_from_slice(&encoded);
    Ok(())
}

fn command_words(command: AbiCommand) -> (AbiId, u16, [u64; 6]) {
    match command {
        AbiCommand::Copy(copy) => (
            copy.command_id(),
            COPY_OPCODE,
            [
                copy.source().raw(),
                copy.destination().raw(),
                u64::from(copy.length()),
                0,
                0,
                0,
            ],
        ),
        AbiCommand::StridedCopy(copy) => (
            copy.command_id(),
            STRIDED_COPY_OPCODE,
            [
                copy.source().raw(),
                copy.destination().raw(),
                u64::from(copy.rows()),
                u64::from(copy.row_bytes()),
                u64::from(copy.source_stride()),
                u64::from(copy.destination_stride()),
            ],
        ),
        AbiCommand::BroadcastCopy(copy) => (
            copy.command_id(),
            BROADCAST_COPY_OPCODE,
            [
                copy.source().raw(),
                copy.destination_list().raw(),
                u64::from(copy.length()),
                u64::from(copy.destination_count()),
                0,
                0,
            ],
        ),
        AbiCommand::VectorAdd(add) => (
            add.command_id(),
            VECTOR_ADD_OPCODE,
            [
                add.source_a().raw(),
                add.source_b().raw(),
                add.destination().raw(),
                u64::from(add.length()),
                0,
                0,
            ],
        ),
    }
}
