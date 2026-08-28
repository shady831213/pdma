mod conversion;
mod error;
mod location;
mod schema;
mod violation;

use super::{ABI_VERSION, AbiEncodeError, DmaAddress, INDIRECT_ENTRY_ALIGNMENT};

pub use error::DescriptorDecodeError;
pub(in crate::abi) use location::{
    canonical_value_location, header_rank, is_indirect_alignment_location,
    is_indirect_count_location, is_narrowing_location, is_reserved_location, is_scalar_location,
    location_offset, scalar_opcode, scalar_rank,
};
pub(in crate::abi) use violation::DescriptorViolationValue;
pub use violation::{DescriptorViolation, DescriptorViolationKind};

use error::DescriptorPayloadViolation;

pub use schema::{
    AbiCommand, AbiCommandHeader, AbiCommandHeaderFlagsRaw, AbiCommandHeaderFlagsValues,
    AbiCommandHeaderOpcode, AbiCommandHeaderOpcodeRaw, AbiCommandHeaderRaw, AbiCommandHeaderView,
    AbiCommandHeaderViewMut, AbiCommandPayloadBroadcastCopyValues, AbiCommandPayloadCopyValues,
    AbiCommandPayloadRaw, AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues,
    AbiCommandPayloadVectorAddValues, AbiCommandRaw, AbiCommandView, AbiCommandViewMut,
    IndirectDestinationEntrySchema, IndirectDestinationEntrySchemaRaw,
    IndirectDestinationEntrySchemaView, IndirectDestinationEntrySchemaViewMut, U32Word, U32WordRaw,
    U32WordValues,
};

/// Canonical generated value representation of one indirect destination entry.
pub type BroadcastDestinationEntry = IndirectDestinationEntrySchema;

const DESTINATION_ENTRY_SIZE: usize = IndirectDestinationEntrySchema::SPEC.size();

impl AbiCommand {
    /// Creates a command from its identifier and generated payload case.
    pub const fn from_payload(command_id: u64, payload: AbiCommandPayloadValues) -> Self {
        Self::new(
            AbiCommandHeader::new(
                ABI_VERSION,
                payload.tag(),
                AbiCommandHeaderFlagsValues::ZERO,
                command_id,
            ),
            payload,
        )
    }

    /// Encodes one command into a complete 64-byte descriptor.
    ///
    /// # Errors
    /// Returns a typed error when `bytes` is shorter than one descriptor.
    pub fn encode(self, bytes: &mut [u8]) -> Result<(), AbiEncodeError> {
        let actual = bytes.len();
        let destination =
            bytes
                .get_mut(..Self::RECORD_SIZE)
                .ok_or(AbiEncodeError::BufferTooShort {
                    required: Self::RECORD_SIZE,
                    actual,
                })?;
        let raw = AbiCommandRaw::try_from(self).map_err(super::codec::encode_schema_error)?;
        destination.copy_from_slice(raw.as_bytes());
        Ok(())
    }

    /// Decodes and validates one complete ABI-v1 descriptor.
    ///
    /// # Errors
    /// Returns the first canonical validation failure.
    pub fn decode(bytes: &[u8]) -> Result<Self, DescriptorDecodeError> {
        let actual = bytes.len();
        let source = bytes.get(..Self::RECORD_SIZE).ok_or_else(|| {
            DescriptorDecodeError::Codec(super::codec::decode_schema_error(
                hardware_interface_schema::SchemaCodecError::BufferTooShort {
                    required: Self::RECORD_SIZE,
                    actual,
                },
            ))
        })?;
        let mut raw = [0_u8; Self::RECORD_SIZE];
        raw.copy_from_slice(source);
        conversion::decode_validated(AbiCommandRaw::from(raw))
    }

    /// Fixed-width command identifier.
    pub const fn command_id(self) -> u64 {
        self.header.command_id
    }
}

impl IndirectDestinationEntrySchema {
    /// Encodes one destination DMA address into an eight-byte list entry.
    ///
    /// # Errors
    /// Returns a typed error when `bytes` is shorter than one entry.
    pub fn encode(self, bytes: &mut [u8]) -> Result<(), AbiEncodeError> {
        let actual = bytes.len();
        let destination =
            bytes
                .get_mut(..DESTINATION_ENTRY_SIZE)
                .ok_or(AbiEncodeError::BufferTooShort {
                    required: DESTINATION_ENTRY_SIZE,
                    actual,
                })?;
        let raw = IndirectDestinationEntrySchemaRaw::try_from(self)
            .map_err(super::codec::encode_schema_error)?;
        destination.copy_from_slice(raw.as_bytes());
        Ok(())
    }

    /// Decodes one eight-byte destination DMA address.
    ///
    /// # Errors
    /// Returns a typed error when `bytes` is shorter than one entry.
    pub fn decode(bytes: &[u8]) -> Result<Self, DescriptorDecodeError> {
        let actual = bytes.len();
        let source = bytes.get(..DESTINATION_ENTRY_SIZE).ok_or_else(|| {
            DescriptorDecodeError::Codec(super::codec::decode_schema_error(
                hardware_interface_schema::SchemaCodecError::BufferTooShort {
                    required: DESTINATION_ENTRY_SIZE,
                    actual,
                },
            ))
        })?;
        let mut raw = [0_u8; DESTINATION_ENTRY_SIZE];
        raw.copy_from_slice(source);
        Self::try_from(IndirectDestinationEntrySchemaRaw::from(raw))
            .map_err(super::codec::decode_schema_error)
            .map_err(DescriptorDecodeError::Codec)
    }

    /// Creates one entry from an absolute DMA address.
    pub const fn from_address(address: DmaAddress) -> Self {
        Self {
            destination_dma_address: address.raw(),
        }
    }

    /// Absolute destination DMA address.
    pub const fn address(self) -> DmaAddress {
        DmaAddress::new(self.destination_dma_address)
    }
}

const fn validate_payload(
    payload: AbiCommandPayloadValues,
) -> Result<(), DescriptorPayloadViolation> {
    match payload {
        AbiCommandPayloadValues::Copy(_)
        | AbiCommandPayloadValues::StridedCopy(_)
        | AbiCommandPayloadValues::VectorAdd(_) => Ok(()),
        AbiCommandPayloadValues::BroadcastCopy(values) => {
            if values.destination_count.value == 0 {
                return Err(DescriptorPayloadViolation::InvalidIndirectCount {
                    list: values.destination_list_dma_address,
                    observed: values.destination_count.value,
                });
            }
            if values.destination_list_dma_address % INDIRECT_ENTRY_ALIGNMENT != 0 {
                return Err(DescriptorPayloadViolation::InvalidIndirectAlignment {
                    address: values.destination_list_dma_address,
                });
            }
            Ok(())
        }
    }
}
