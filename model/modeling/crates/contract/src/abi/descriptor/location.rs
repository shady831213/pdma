use hardware_interface_schema::{SchemaCoordinate, SchemaLocation, SchemaMemberIdentity};

use super::{
    DescriptorHeaderSchema, DescriptorHeaderSchemaOpcode, DescriptorSchema,
    DescriptorSchemaPayloadBroadcastCopyValues, DescriptorSchemaPayloadCopyValues,
    DescriptorSchemaPayloadStridedCopyValues, DescriptorSchemaPayloadVectorAddValues,
};

pub(in crate::abi) const fn location_offset(location: SchemaLocation) -> u64 {
    let offset = match location.coordinate() {
        SchemaCoordinate::Byte(offset) => offset,
        SchemaCoordinate::Bit(offset) => offset / 8,
    };
    const { assert!(usize::BITS <= u64::BITS) };
    offset as u64
}

pub(in crate::abi) fn header_rank(location: SchemaLocation) -> Option<u64> {
    if location
        == DescriptorHeaderSchema::VERSION_IDENTITY.at_byte(DescriptorHeaderSchema::VERSION_OFFSET)
    {
        Some(0)
    } else if location
        == DescriptorHeaderSchema::SIZE_IDENTITY.at_byte(DescriptorHeaderSchema::SIZE_OFFSET)
    {
        Some(1)
    } else if location
        == DescriptorHeaderSchema::OPCODE_IDENTITY.at_byte(DescriptorHeaderSchema::OPCODE_OFFSET)
    {
        Some(2)
    } else if location
        == DescriptorHeaderSchema::FLAGS_IDENTITY.at_byte(DescriptorHeaderSchema::FLAGS_OFFSET)
    {
        Some(3)
    } else {
        None
    }
}

pub(in crate::abi) fn is_reserved_location(
    opcode: DescriptorHeaderSchemaOpcode,
    location: SchemaLocation,
) -> bool {
    if location.identity() != DescriptorSchema::PAYLOAD_IDENTITY {
        return false;
    }
    let SchemaCoordinate::Byte(offset) = location.coordinate() else {
        return false;
    };
    match opcode {
        DescriptorHeaderSchemaOpcode::Copy => {
            offset == DescriptorSchema::WORD3_OFFSET
                || offset == DescriptorSchema::WORD4_OFFSET
                || offset == DescriptorSchema::WORD5_OFFSET
        }
        DescriptorHeaderSchemaOpcode::StridedCopy => false,
        DescriptorHeaderSchemaOpcode::BroadcastCopy | DescriptorHeaderSchemaOpcode::VectorAdd => {
            offset == DescriptorSchema::WORD4_OFFSET || offset == DescriptorSchema::WORD5_OFFSET
        }
    }
}

pub(in crate::abi) fn is_narrowing_location(location: SchemaLocation) -> bool {
    is_at(
        location,
        DescriptorSchemaPayloadCopyValues::LENGTH_IDENTITY,
        DescriptorSchema::WORD2_OFFSET,
    ) || is_at(
        location,
        DescriptorSchemaPayloadStridedCopyValues::ROWS_IDENTITY,
        DescriptorSchema::WORD2_OFFSET,
    ) || is_at(
        location,
        DescriptorSchemaPayloadStridedCopyValues::ROW_BYTES_IDENTITY,
        DescriptorSchema::WORD3_OFFSET,
    ) || is_at(
        location,
        DescriptorSchemaPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY,
        DescriptorSchema::WORD4_OFFSET,
    ) || is_at(
        location,
        DescriptorSchemaPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY,
        DescriptorSchema::WORD5_OFFSET,
    ) || is_at(
        location,
        DescriptorSchemaPayloadBroadcastCopyValues::LENGTH_IDENTITY,
        DescriptorSchema::WORD2_OFFSET,
    ) || is_at(
        location,
        DescriptorSchemaPayloadVectorAddValues::LENGTH_IDENTITY,
        DescriptorSchema::WORD3_OFFSET,
    )
}

pub(in crate::abi) fn is_scalar_location(location: SchemaLocation) -> bool {
    is_narrowing_location(location)
        || is_indirect_count_location(location)
        || is_indirect_alignment_location(location)
}

pub(in crate::abi) fn is_indirect_count_location(location: SchemaLocation) -> bool {
    is_at(
        location,
        DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY,
        DescriptorSchema::WORD3_OFFSET,
    )
}

pub(in crate::abi) fn is_indirect_alignment_location(location: SchemaLocation) -> bool {
    is_at(
        location,
        DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY,
        DescriptorSchema::WORD1_OFFSET,
    )
}

pub(in crate::abi) fn scalar_opcode(
    location: SchemaLocation,
) -> Option<DescriptorHeaderSchemaOpcode> {
    let identity = location.identity();
    if identity == DescriptorSchemaPayloadCopyValues::LENGTH_IDENTITY {
        Some(DescriptorHeaderSchemaOpcode::Copy)
    } else if identity == DescriptorSchemaPayloadStridedCopyValues::ROWS_IDENTITY
        || identity == DescriptorSchemaPayloadStridedCopyValues::ROW_BYTES_IDENTITY
        || identity == DescriptorSchemaPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY
        || identity == DescriptorSchemaPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY
    {
        Some(DescriptorHeaderSchemaOpcode::StridedCopy)
    } else if identity == DescriptorSchemaPayloadBroadcastCopyValues::LENGTH_IDENTITY
        || identity == DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
        || identity
            == DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
    {
        Some(DescriptorHeaderSchemaOpcode::BroadcastCopy)
    } else if identity == DescriptorSchemaPayloadVectorAddValues::LENGTH_IDENTITY {
        Some(DescriptorHeaderSchemaOpcode::VectorAdd)
    } else {
        None
    }
}

pub(in crate::abi) fn scalar_rank(location: SchemaLocation) -> Option<u64> {
    if !is_scalar_location(location) {
        return None;
    }
    if location.identity()
        == DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
    {
        Some(location_offset(
            DescriptorSchema::PAYLOAD_IDENTITY.at_byte(DescriptorSchema::WORD4_OFFSET),
        ))
    } else {
        Some(location_offset(location))
    }
}

fn is_at(location: SchemaLocation, identity: SchemaMemberIdentity, offset: usize) -> bool {
    location.identity() == identity
        && matches!(location.coordinate(), SchemaCoordinate::Byte(actual) if actual == offset)
}
