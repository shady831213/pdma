use crate::abi::{
    ABI_VERSION, AbiEncodeError, AbiId, BROADCAST_COPY_OPCODE, COPY_OPCODE, DESCRIPTOR,
    DESCRIPTOR_FLAGS_MASK, FieldSpec, STRIDED_COPY_OPCODE, VECTOR_ADD_OPCODE, write_u16_le,
    write_u64_le,
};

use super::{AbiCommand, BroadcastDestinationEntry, DescriptorDecodeError};

#[derive(Clone, Copy)]
pub(super) struct DescriptorFields {
    pub(super) version: FieldSpec,
    pub(super) opcode: FieldSpec,
    pub(super) flags: FieldSpec,
    pub(super) size: FieldSpec,
    pub(super) command_id: FieldSpec,
    pub(super) words: [FieldSpec; 6],
}

pub(super) fn encode_descriptor(
    command: AbiCommand,
    output: &mut [u8],
) -> Result<(), AbiEncodeError> {
    let required = usize::from(DESCRIPTOR.size());
    let actual = output.len();
    let destination = output
        .get_mut(..required)
        .ok_or(AbiEncodeError::BufferTooShort { required, actual })?;
    let (command_id, opcode, words) = command_words(command);
    let fields = descriptor_fields().map_err(|_| AbiEncodeError::InvalidFieldRange {
        start: 0,
        end: 0,
        width: required,
    })?;
    let mut encoded = [0_u8; 64];
    write_u16_le(&mut encoded, fields.version.range(), ABI_VERSION)?;
    write_u16_le(&mut encoded, fields.opcode.range(), opcode)?;
    write_u16_le(&mut encoded, fields.flags.range(), DESCRIPTOR_FLAGS_MASK)?;
    write_u16_le(&mut encoded, fields.size.range(), DESCRIPTOR.size())?;
    write_u64_le(&mut encoded, fields.command_id.range(), command_id.raw())?;
    for (field, word) in fields.words.iter().zip(words) {
        write_u64_le(&mut encoded, field.range(), word)?;
    }
    destination.copy_from_slice(&encoded);
    Ok(())
}

pub(super) fn encode_destination(
    entry: BroadcastDestinationEntry,
    bytes: &mut [u8],
) -> Result<(), AbiEncodeError> {
    write_u64_le(bytes, 0..8, entry.address().raw())
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

pub(super) fn descriptor_fields() -> Result<DescriptorFields, DescriptorDecodeError> {
    let &[
        version,
        opcode,
        flags,
        size,
        command_id,
        word0,
        word1,
        word2,
        word3,
        word4,
        word5,
    ] = DESCRIPTOR.fields()
    else {
        return Err(DescriptorDecodeError::MetadataMismatch);
    };
    Ok(DescriptorFields {
        version,
        opcode,
        flags,
        size,
        command_id,
        words: [word0, word1, word2, word3, word4, word5],
    })
}
