use crate::abi::{
    ABI_VERSION, AbiEncodeError, AbiId, BROADCAST_COPY_OPCODE, COPY_OPCODE, DESCRIPTOR_FLAGS_MASK,
    DESCRIPTOR_SIZE, DescriptorSchema, DescriptorSchemaViewMut, INDIRECT_DESTINATION_ENTRY_SIZE,
    IndirectDestinationEntrySchema, IndirectDestinationEntrySchemaViewMut, STRIDED_COPY_OPCODE,
    VECTOR_ADD_OPCODE, codec::encode_schema_error,
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
    let mut encoded = [0_u8; DescriptorSchema::SPEC.size()];
    let mut view = DescriptorSchemaViewMut::new(&mut encoded).map_err(encode_schema_error)?;
    view.set_version(ABI_VERSION).map_err(encode_schema_error)?;
    view.set_opcode(opcode).map_err(encode_schema_error)?;
    view.set_flags(DESCRIPTOR_FLAGS_MASK)
        .map_err(encode_schema_error)?;
    view.set_size(DESCRIPTOR_SIZE)
        .map_err(encode_schema_error)?;
    view.set_command_id(command_id.raw())
        .map_err(encode_schema_error)?;
    view.set_word0(words[0]).map_err(encode_schema_error)?;
    view.set_word1(words[1]).map_err(encode_schema_error)?;
    view.set_word2(words[2]).map_err(encode_schema_error)?;
    view.set_word3(words[3]).map_err(encode_schema_error)?;
    view.set_word4(words[4]).map_err(encode_schema_error)?;
    view.set_word5(words[5]).map_err(encode_schema_error)?;
    destination.copy_from_slice(&encoded);
    Ok(())
}

pub(super) fn encode_destination(
    entry: BroadcastDestinationEntry,
    bytes: &mut [u8],
) -> Result<(), AbiEncodeError> {
    let required = usize::from(INDIRECT_DESTINATION_ENTRY_SIZE);
    let actual = bytes.len();
    let mut encoded = [0_u8; IndirectDestinationEntrySchema::SPEC.size()];
    let mut view =
        IndirectDestinationEntrySchemaViewMut::new(&mut encoded).map_err(encode_schema_error)?;
    view.set_destination_iova(entry.address().raw())
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
