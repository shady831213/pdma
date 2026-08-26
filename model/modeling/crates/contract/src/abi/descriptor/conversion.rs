mod classify;

use crate::abi::{
    ABI_VERSION, AbiEncodeError, COPY_OPCODE, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE,
    DescriptorHeaderSchemaOpcode, DescriptorHeaderSchemaView, DescriptorSchemaRaw,
    DescriptorSchemaValues, DescriptorSchemaView, DmaAddress, VECTOR_ADD_OPCODE,
    codec::decode_schema_error,
};

use self::classify::{DescriptorContext, descriptor_schema_error};
use super::{AbiCommand, DescriptorDecodeError, DescriptorViolation, validate_payload};

impl TryFrom<DescriptorSchemaRaw> for AbiCommand {
    type Error = DescriptorDecodeError;

    fn try_from(raw: DescriptorSchemaRaw) -> Result<Self, Self::Error> {
        let view = DescriptorSchemaView::new(raw.as_bytes()).map_err(decode_schema_error)?;
        let header = view.header().map_err(decode_schema_error)?;
        let opcode = validate_header(header)?;
        let list = DmaAddress::new(view.word1().map_err(decode_schema_error)?);
        let context = DescriptorContext::new(opcode, list);
        let values = DescriptorSchemaValues::try_from(raw)
            .map_err(|error| descriptor_schema_error(context, error))?;
        validate_payload(values.payload)
            .map_err(DescriptorViolation::from)
            .map_err(DescriptorDecodeError::Violation)?;
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
    header: DescriptorHeaderSchemaView<'_>,
) -> Result<DescriptorHeaderSchemaOpcode, DescriptorDecodeError> {
    let version = header.version().map_err(decode_schema_error)?;
    if version != ABI_VERSION {
        return Err(DescriptorDecodeError::Violation(
            DescriptorViolation::version(version),
        ));
    }
    let size = header.size().map_err(decode_schema_error)?;
    if size != DESCRIPTOR_SIZE {
        return Err(DescriptorDecodeError::Violation(DescriptorViolation::size(
            size,
        )));
    }
    let opcode = header.opcode().map_err(decode_schema_error)?;
    if !(COPY_OPCODE..=VECTOR_ADD_OPCODE).contains(&opcode) {
        return Err(DescriptorDecodeError::Violation(
            DescriptorViolation::opcode(opcode),
        ));
    }
    let flags = header.flags().map_err(decode_schema_error)?;
    if flags != DESCRIPTOR_FLAGS_MASK {
        return Err(DescriptorDecodeError::Violation(
            DescriptorViolation::flags(flags),
        ));
    }
    DescriptorHeaderSchemaOpcode::from_raw(opcode)
        .map_err(|_| DescriptorDecodeError::MetadataMismatch)
}
