use crate::abi::{
    ABI_VERSION, AbiEncodeError, DescriptorHeaderSchemaFlagsValues, DescriptorHeaderSchemaOpcode,
    DescriptorHeaderSchemaValues, DescriptorSchema, DescriptorSchemaPayloadValues,
    DescriptorSchemaValues, INDIRECT_DESTINATION_ENTRY_SIZE, IndirectDestinationEntrySchemaValues,
    codec::encode_schema_error,
};

use super::{AbiCommand, BroadcastDestinationEntry};

pub(super) fn encode_descriptor(
    command: AbiCommand,
    output: &mut [u8],
) -> Result<(), AbiEncodeError> {
    let required = DescriptorSchema::RECORD_SIZE;
    let actual = output.len();
    let destination = output
        .get_mut(..required)
        .ok_or(AbiEncodeError::BufferTooShort { required, actual })?;
    let (command_id, opcode, payload) = match command {
        AbiCommand::Copy(copy) => (
            copy.command_id().raw(),
            DescriptorHeaderSchemaOpcode::Copy,
            DescriptorSchemaPayloadValues::Copy {
                source_dma_address: copy.source().raw(),
                destination_dma_address: copy.destination().raw(),
                length: copy.length(),
            },
        ),
        AbiCommand::StridedCopy(copy) => (
            copy.command_id().raw(),
            DescriptorHeaderSchemaOpcode::StridedCopy,
            DescriptorSchemaPayloadValues::StridedCopy {
                source_dma_address: copy.source().raw(),
                destination_dma_address: copy.destination().raw(),
                rows: copy.rows(),
                row_bytes: copy.row_bytes(),
                source_stride: copy.source_stride(),
                destination_stride: copy.destination_stride(),
            },
        ),
        AbiCommand::BroadcastCopy(copy) => (
            copy.command_id().raw(),
            DescriptorHeaderSchemaOpcode::BroadcastCopy,
            DescriptorSchemaPayloadValues::BroadcastCopy {
                source_dma_address: copy.source().raw(),
                destination_list_dma_address: copy.destination_list().raw(),
                length: copy.length(),
                destination_count: copy.destination_count(),
            },
        ),
        AbiCommand::VectorAdd(add) => (
            add.command_id().raw(),
            DescriptorHeaderSchemaOpcode::VectorAdd,
            DescriptorSchemaPayloadValues::VectorAdd {
                source_a_dma_address: add.source_a().raw(),
                source_b_dma_address: add.source_b().raw(),
                destination_dma_address: add.destination().raw(),
                length: add.length(),
            },
        ),
    };
    let encoded = DescriptorSchemaValues {
        header: DescriptorHeaderSchemaValues {
            version: ABI_VERSION,
            opcode,
            flags: DescriptorHeaderSchemaFlagsValues::ZERO,
            command_id,
        },
        payload,
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
        destination_dma_address: entry.address().raw(),
    }
    .to_bytes()
    .map_err(encode_schema_error)?;
    let destination = bytes
        .get_mut(..required)
        .ok_or(AbiEncodeError::BufferTooShort { required, actual })?;
    destination.copy_from_slice(&encoded);
    Ok(())
}
