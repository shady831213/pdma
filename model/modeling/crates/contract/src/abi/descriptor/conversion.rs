use hardware_interface_schema::{SchemaCodecError, SchemaCoordinate, SchemaLocation};

use crate::abi::{
    ABI_VERSION, AbiEncodeError, COPY_OPCODE, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DescriptorSchemaPayloadBroadcastCopyValues, DescriptorSchemaRaw, DescriptorSchemaValues,
    DescriptorSchemaView, MAX_INDIRECT_DESTINATIONS, VECTOR_ADD_OPCODE, codec::decode_schema_error,
};

use super::{AbiCommand, DescriptorDecodeError, validate_payload};

impl TryFrom<DescriptorSchemaRaw> for AbiCommand {
    type Error = DescriptorDecodeError;

    fn try_from(raw: DescriptorSchemaRaw) -> Result<Self, Self::Error> {
        let view = DescriptorSchemaView::new(raw.as_bytes()).map_err(decode_schema_error)?;
        let header = view.header().map_err(decode_schema_error)?;
        let version = header.version().map_err(decode_schema_error)?;
        let size = header.size().map_err(decode_schema_error)?;
        let opcode = header.opcode().map_err(decode_schema_error)?;
        let flags = header.flags().map_err(decode_schema_error)?;
        validate_header(version, size, opcode, flags)?;
        let values = DescriptorSchemaValues::try_from(raw).map_err(descriptor_schema_error)?;
        validate_payload(values.payload).map_err(DescriptorDecodeError::from)?;
        Ok(Self::from_validated_values(values))
    }
}

impl TryFrom<AbiCommand> for DescriptorSchemaRaw {
    type Error = AbiEncodeError;

    fn try_from(command: AbiCommand) -> Result<Self, Self::Error> {
        Self::try_from(command.values()).map_err(crate::abi::codec::encode_schema_error)
    }
}

fn validate_header(
    version: u16,
    size: u16,
    opcode: u16,
    flags: u16,
) -> Result<(), DescriptorDecodeError> {
    if version != ABI_VERSION {
        return Err(DescriptorDecodeError::InvalidVersion {
            observed: version,
            expected: ABI_VERSION,
        });
    }
    if size != DESCRIPTOR_SIZE {
        return Err(DescriptorDecodeError::InvalidSize {
            observed: size,
            expected: DESCRIPTOR_SIZE,
        });
    }
    if !(COPY_OPCODE..=VECTOR_ADD_OPCODE).contains(&opcode) {
        return Err(DescriptorDecodeError::InvalidOpcode {
            observed: opcode,
            maximum: VECTOR_ADD_OPCODE,
        });
    }
    if flags != DESCRIPTOR_FLAGS_MASK {
        return Err(DescriptorDecodeError::InvalidFlags {
            observed: flags,
            allowed: DESCRIPTOR_FLAGS_MASK,
        });
    }
    Ok(())
}

fn descriptor_schema_error(error: SchemaCodecError) -> DescriptorDecodeError {
    match error {
        SchemaCodecError::BufferTooShort { .. } => {
            DescriptorDecodeError::Codec(decode_schema_error(error))
        }
        SchemaCodecError::UnknownTag { observed, .. } => {
            let Ok(observed) = u16::try_from(observed) else {
                return DescriptorDecodeError::MetadataMismatch;
            };
            DescriptorDecodeError::InvalidOpcode {
                observed,
                maximum: VECTOR_ADD_OPCODE,
            }
        }
        SchemaCodecError::NonZeroReserved { location, observed } => location_offset(location)
            .map_or(DescriptorDecodeError::MetadataMismatch, |offset| {
                DescriptorDecodeError::InvalidReserved { offset, observed }
            }),
        SchemaCodecError::NarrowingOverflow {
            location,
            observed,
            maximum,
        } => {
            if location.identity()
                == DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
            {
                return DescriptorDecodeError::InvalidIndirectCount {
                    observed,
                    maximum: MAX_INDIRECT_DESTINATIONS,
                };
            }
            let Ok(offset) = location_offset(location) else {
                return DescriptorDecodeError::MetadataMismatch;
            };
            let Ok(maximum) = u32::try_from(maximum) else {
                return DescriptorDecodeError::MetadataMismatch;
            };
            DescriptorDecodeError::ValueOutOfRange {
                offset,
                observed,
                maximum,
            }
        }
        SchemaCodecError::ConstantMismatch { .. }
        | SchemaCodecError::CoordinateRebaseOverflow { .. } => {
            DescriptorDecodeError::MetadataMismatch
        }
        _ => DescriptorDecodeError::MetadataMismatch,
    }
}

fn location_offset(location: SchemaLocation) -> Result<u16, DescriptorDecodeError> {
    let SchemaCoordinate::Byte(offset) = location.coordinate() else {
        return Err(DescriptorDecodeError::MetadataMismatch);
    };
    field_offset(offset)
}

fn field_offset(offset: usize) -> Result<u16, DescriptorDecodeError> {
    u16::try_from(offset).map_err(|_| DescriptorDecodeError::MetadataMismatch)
}
