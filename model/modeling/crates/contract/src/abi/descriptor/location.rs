use hardware_interface_schema::{SchemaCoordinate, SchemaLocation, SchemaMemberIdentity};

use super::{
    AbiCommand, AbiCommandHeader, AbiCommandHeaderOpcode, AbiCommandPayloadBroadcastCopyValues,
    AbiCommandPayloadCopyValues, AbiCommandPayloadStridedCopyValues,
    AbiCommandPayloadVectorAddValues,
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
    if location == AbiCommandHeader::VERSION_IDENTITY.at_byte(AbiCommandHeader::VERSION_OFFSET) {
        Some(0)
    } else if location == AbiCommandHeader::SIZE_IDENTITY.at_byte(AbiCommandHeader::SIZE_OFFSET) {
        Some(1)
    } else if location == AbiCommandHeader::OPCODE_IDENTITY.at_byte(AbiCommandHeader::OPCODE_OFFSET)
    {
        Some(2)
    } else if location == AbiCommandHeader::FLAGS_IDENTITY.at_byte(AbiCommandHeader::FLAGS_OFFSET) {
        Some(3)
    } else {
        None
    }
}

pub(in crate::abi) fn is_reserved_location(
    opcode: AbiCommandHeaderOpcode,
    location: SchemaLocation,
) -> bool {
    if location.identity() != AbiCommand::PAYLOAD_IDENTITY {
        return false;
    }
    let SchemaCoordinate::Byte(offset) = location.coordinate() else {
        return false;
    };
    match opcode {
        AbiCommandHeaderOpcode::Copy => {
            offset == AbiCommand::WORD3_OFFSET
                || offset == AbiCommand::WORD4_OFFSET
                || offset == AbiCommand::WORD5_OFFSET
        }
        AbiCommandHeaderOpcode::StridedCopy => false,
        AbiCommandHeaderOpcode::BroadcastCopy | AbiCommandHeaderOpcode::VectorAdd => {
            offset == AbiCommand::WORD4_OFFSET || offset == AbiCommand::WORD5_OFFSET
        }
    }
}

pub(in crate::abi) fn is_narrowing_location(location: SchemaLocation) -> bool {
    is_at(
        location,
        AbiCommandPayloadCopyValues::LENGTH_IDENTITY,
        AbiCommand::WORD2_OFFSET,
    ) || is_at(
        location,
        AbiCommandPayloadStridedCopyValues::ROWS_IDENTITY,
        AbiCommand::WORD2_OFFSET,
    ) || is_at(
        location,
        AbiCommandPayloadStridedCopyValues::ROW_BYTES_IDENTITY,
        AbiCommand::WORD3_OFFSET,
    ) || is_at(
        location,
        AbiCommandPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY,
        AbiCommand::WORD4_OFFSET,
    ) || is_at(
        location,
        AbiCommandPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY,
        AbiCommand::WORD5_OFFSET,
    ) || is_at(
        location,
        AbiCommandPayloadBroadcastCopyValues::LENGTH_IDENTITY,
        AbiCommand::WORD2_OFFSET,
    ) || is_at(
        location,
        AbiCommandPayloadVectorAddValues::LENGTH_IDENTITY,
        AbiCommand::WORD3_OFFSET,
    )
}

pub(in crate::abi) fn canonical_value_location(location: SchemaLocation) -> Option<SchemaLocation> {
    let identity = location.identity();
    if identity == AbiCommandPayloadCopyValues::LENGTH_IDENTITY
        || identity == AbiCommandPayloadStridedCopyValues::ROWS_IDENTITY
        || identity == AbiCommandPayloadBroadcastCopyValues::LENGTH_IDENTITY
    {
        Some(identity.at_byte(AbiCommand::WORD2_OFFSET))
    } else if identity == AbiCommandPayloadStridedCopyValues::ROW_BYTES_IDENTITY
        || identity == AbiCommandPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
        || identity == AbiCommandPayloadVectorAddValues::LENGTH_IDENTITY
    {
        Some(identity.at_byte(AbiCommand::WORD3_OFFSET))
    } else if identity == AbiCommandPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY {
        Some(identity.at_byte(AbiCommand::WORD4_OFFSET))
    } else if identity == AbiCommandPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY {
        Some(identity.at_byte(AbiCommand::WORD5_OFFSET))
    } else {
        None
    }
}

pub(in crate::abi) fn is_scalar_location(location: SchemaLocation) -> bool {
    is_narrowing_location(location)
        || is_indirect_count_location(location)
        || is_indirect_alignment_location(location)
}

pub(in crate::abi) fn is_indirect_count_location(location: SchemaLocation) -> bool {
    is_at(
        location,
        AbiCommandPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY,
        AbiCommand::WORD3_OFFSET,
    )
}

pub(in crate::abi) fn is_indirect_alignment_location(location: SchemaLocation) -> bool {
    is_at(
        location,
        AbiCommandPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY,
        AbiCommand::WORD1_OFFSET,
    )
}

pub(in crate::abi) fn scalar_opcode(location: SchemaLocation) -> Option<AbiCommandHeaderOpcode> {
    let identity = location.identity();
    if identity == AbiCommandPayloadCopyValues::LENGTH_IDENTITY {
        Some(AbiCommandHeaderOpcode::Copy)
    } else if identity == AbiCommandPayloadStridedCopyValues::ROWS_IDENTITY
        || identity == AbiCommandPayloadStridedCopyValues::ROW_BYTES_IDENTITY
        || identity == AbiCommandPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY
        || identity == AbiCommandPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY
    {
        Some(AbiCommandHeaderOpcode::StridedCopy)
    } else if identity == AbiCommandPayloadBroadcastCopyValues::LENGTH_IDENTITY
        || identity == AbiCommandPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
        || identity == AbiCommandPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
    {
        Some(AbiCommandHeaderOpcode::BroadcastCopy)
    } else if identity == AbiCommandPayloadVectorAddValues::LENGTH_IDENTITY {
        Some(AbiCommandHeaderOpcode::VectorAdd)
    } else {
        None
    }
}

pub(in crate::abi) fn scalar_rank(location: SchemaLocation) -> Option<u64> {
    if !is_scalar_location(location) {
        return None;
    }
    if location.identity()
        == AbiCommandPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
    {
        Some(location_offset(
            AbiCommand::PAYLOAD_IDENTITY.at_byte(AbiCommand::WORD4_OFFSET),
        ))
    } else {
        Some(location_offset(location))
    }
}

fn is_at(location: SchemaLocation, identity: SchemaMemberIdentity, offset: usize) -> bool {
    location.identity() == identity
        && matches!(location.coordinate(), SchemaCoordinate::Byte(actual) if actual == offset)
}
