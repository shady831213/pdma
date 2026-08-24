mod codec;
mod decode;
mod error;
mod schema;

use super::{
    ABI_VERSION, AbiEncodeError, AbiId, DmaAddress, INDIRECT_ENTRY_ALIGNMENT,
    MAX_INDIRECT_DESTINATIONS,
};

pub use error::{DescriptorDecodeError, DescriptorValueError};

pub use schema::{
    DescriptorHeaderSchema, DescriptorHeaderSchemaFlagsValues, DescriptorHeaderSchemaOpcode,
    DescriptorHeaderSchemaValues, DescriptorHeaderSchemaView, DescriptorHeaderSchemaViewMut,
    DescriptorSchema, DescriptorSchemaPayloadBroadcastCopyValues,
    DescriptorSchemaPayloadCopyValues, DescriptorSchemaPayloadStridedCopyValues,
    DescriptorSchemaPayloadValues, DescriptorSchemaPayloadVectorAddValues, DescriptorSchemaValues,
    DescriptorSchemaView, DescriptorSchemaViewMut, IndirectDestinationEntrySchema,
    IndirectDestinationEntrySchemaValues, IndirectDestinationEntrySchemaView,
    IndirectDestinationEntrySchemaViewMut,
};

/// One validated ABI-v1 command backed by the generated descriptor value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiCommand(DescriptorSchemaValues);

/// Canonical generated value representation of one indirect destination entry.
pub type BroadcastDestinationEntry = IndirectDestinationEntrySchemaValues;

impl AbiCommand {
    /// Creates a canonical linear-copy descriptor.
    pub const fn copy(command_id: AbiId, values: DescriptorSchemaPayloadCopyValues) -> Self {
        Self::from_payload(command_id, DescriptorSchemaPayloadValues::Copy(values))
    }

    /// Creates a canonical strided-copy descriptor.
    pub const fn strided_copy(
        command_id: AbiId,
        values: DescriptorSchemaPayloadStridedCopyValues,
    ) -> Self {
        Self::from_payload(
            command_id,
            DescriptorSchemaPayloadValues::StridedCopy(values),
        )
    }

    /// Creates a canonical indirect-broadcast descriptor.
    ///
    /// # Errors
    /// Returns a typed error when the destination list is empty or unaligned.
    pub fn try_broadcast_copy(
        command_id: AbiId,
        values: DescriptorSchemaPayloadBroadcastCopyValues,
    ) -> Result<Self, DescriptorValueError> {
        let payload = DescriptorSchemaPayloadValues::BroadcastCopy(values);
        validate_payload(payload)?;
        Ok(Self::from_payload(command_id, payload))
    }

    /// Creates a canonical vector-add descriptor.
    pub const fn vector_add(
        command_id: AbiId,
        values: DescriptorSchemaPayloadVectorAddValues,
    ) -> Self {
        Self::from_payload(command_id, DescriptorSchemaPayloadValues::VectorAdd(values))
    }

    const fn from_payload(command_id: AbiId, payload: DescriptorSchemaPayloadValues) -> Self {
        Self(DescriptorSchemaValues {
            header: DescriptorHeaderSchemaValues {
                version: ABI_VERSION,
                opcode: payload.tag(),
                flags: DescriptorHeaderSchemaFlagsValues::ZERO,
                command_id: command_id.raw(),
            },
            payload,
        })
    }

    const fn from_validated_values(values: DescriptorSchemaValues) -> Self {
        Self(values)
    }

    /// Encodes one command into a complete 64-byte descriptor.
    ///
    /// # Errors
    /// Returns a typed error when `bytes` is shorter than one descriptor.
    pub fn encode(self, bytes: &mut [u8]) -> Result<(), AbiEncodeError> {
        codec::encode_descriptor(self, bytes)
    }

    /// Decodes and validates one complete ABI-v1 descriptor.
    ///
    /// # Errors
    /// Returns the first canonical validation failure.
    pub fn decode(bytes: &[u8]) -> Result<Self, DescriptorDecodeError> {
        decode::decode_descriptor(bytes)
    }

    /// Fixed-width command identifier.
    pub const fn command_id(self) -> AbiId {
        AbiId::new(self.0.header.command_id)
    }

    /// Generated payload selected by the descriptor opcode.
    pub const fn payload(self) -> DescriptorSchemaPayloadValues {
        self.0.payload
    }

    /// Complete generated descriptor value.
    pub const fn values(self) -> DescriptorSchemaValues {
        self.0
    }
}

impl IndirectDestinationEntrySchemaValues {
    /// Encodes one destination DMA address into an eight-byte list entry.
    ///
    /// # Errors
    /// Returns a typed error when `bytes` is shorter than one entry.
    pub fn encode(self, bytes: &mut [u8]) -> Result<(), AbiEncodeError> {
        self.encode_into(bytes)
            .map_err(super::codec::encode_schema_error)
    }

    /// Decodes one eight-byte destination DMA address.
    ///
    /// # Errors
    /// Returns a typed error when `bytes` is shorter than one entry.
    pub fn decode(bytes: &[u8]) -> Result<Self, DescriptorDecodeError> {
        Self::from_bytes(bytes)
            .map_err(super::codec::decode_schema_error)
            .map_err(DescriptorDecodeError::Codec)
    }

    /// Creates one entry from an absolute DMA address.
    pub const fn new(address: DmaAddress) -> Self {
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
    payload: DescriptorSchemaPayloadValues,
) -> Result<(), DescriptorValueError> {
    match payload {
        DescriptorSchemaPayloadValues::Copy(_)
        | DescriptorSchemaPayloadValues::StridedCopy(_)
        | DescriptorSchemaPayloadValues::VectorAdd(_) => Ok(()),
        DescriptorSchemaPayloadValues::BroadcastCopy(values) => {
            if values.destination_count == 0 {
                return Err(DescriptorValueError::InvalidIndirectCount {
                    observed: values.destination_count,
                    maximum: MAX_INDIRECT_DESTINATIONS,
                });
            }
            if values.destination_list_dma_address % INDIRECT_ENTRY_ALIGNMENT != 0 {
                return Err(DescriptorValueError::InvalidIndirectAlignment {
                    address: values.destination_list_dma_address,
                    alignment: INDIRECT_ENTRY_ALIGNMENT,
                });
            }
            Ok(())
        }
    }
}
