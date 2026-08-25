mod conversion;
mod error;
mod schema;

use super::{
    ABI_VERSION, AbiEncodeError, AbiId, DmaAddress, INDIRECT_ENTRY_ALIGNMENT,
    MAX_INDIRECT_DESTINATIONS,
};

pub use error::{DescriptorDecodeError, DescriptorValueError};

pub use schema::{
    DescriptorHeaderSchema, DescriptorHeaderSchemaFlagsRaw, DescriptorHeaderSchemaFlagsValues,
    DescriptorHeaderSchemaOpcode, DescriptorHeaderSchemaOpcodeRaw, DescriptorHeaderSchemaRaw,
    DescriptorHeaderSchemaValues, DescriptorHeaderSchemaView, DescriptorHeaderSchemaViewMut,
    DescriptorSchema, DescriptorSchemaPayloadBroadcastCopyValues,
    DescriptorSchemaPayloadCopyValues, DescriptorSchemaPayloadRaw,
    DescriptorSchemaPayloadStridedCopyValues, DescriptorSchemaPayloadValues,
    DescriptorSchemaPayloadVectorAddValues, DescriptorSchemaRaw, DescriptorSchemaValues,
    DescriptorSchemaView, DescriptorSchemaViewMut, IndirectDestinationEntrySchema,
    IndirectDestinationEntrySchemaRaw, IndirectDestinationEntrySchemaValues,
    IndirectDestinationEntrySchemaView, IndirectDestinationEntrySchemaViewMut,
};

/// One validated ABI-v1 command backed by the generated descriptor value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiCommand(DescriptorSchemaValues);

/// Canonical generated value representation of one indirect destination entry.
pub type BroadcastDestinationEntry = IndirectDestinationEntrySchemaValues;

const DESTINATION_ENTRY_SIZE: usize = IndirectDestinationEntrySchema::SPEC.size();

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
        let actual = bytes.len();
        let destination = bytes.get_mut(..DescriptorSchema::RECORD_SIZE).ok_or(
            AbiEncodeError::BufferTooShort {
                required: DescriptorSchema::RECORD_SIZE,
                actual,
            },
        )?;
        let raw = DescriptorSchemaRaw::try_from(self)?;
        destination.copy_from_slice(raw.as_bytes());
        Ok(())
    }

    /// Decodes and validates one complete ABI-v1 descriptor.
    ///
    /// # Errors
    /// Returns the first canonical validation failure.
    pub fn decode(bytes: &[u8]) -> Result<Self, DescriptorDecodeError> {
        let actual = bytes.len();
        let source = bytes.get(..DescriptorSchema::RECORD_SIZE).ok_or_else(|| {
            DescriptorDecodeError::Codec(super::codec::decode_schema_error(
                hardware_interface_schema::SchemaCodecError::BufferTooShort {
                    required: DescriptorSchema::RECORD_SIZE,
                    actual,
                },
            ))
        })?;
        let mut raw = [0_u8; DescriptorSchema::RECORD_SIZE];
        raw.copy_from_slice(source);
        Self::try_from(DescriptorSchemaRaw::from(raw))
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
