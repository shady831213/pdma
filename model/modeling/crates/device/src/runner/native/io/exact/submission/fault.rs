use crate::{ModelError, ModelResult};
use pdma_model_contract::abi::{
    AbiFault, AbiFaultCandidate, AbiFaultOrder, AbiHeaderField, AbiReservedField, AbiScalarField,
    BROADCAST_COPY_OPCODE, COPY_OPCODE, DmaAddress, STRIDED_COPY_OPCODE, VECTOR_ADD_OPCODE,
    select_canonical_fault,
};
use pdma_model_contract::abi::{DescriptorDecodeError, DescriptorSchema, DescriptorSchemaView};

pub(super) fn descriptor(
    slot: DmaAddress,
    bytes: &[u8],
    error: DescriptorDecodeError,
) -> ModelResult<AbiFault> {
    let view = DescriptorSchemaView::new(bytes).map_err(|_| ModelError::DmaSubmissionState)?;
    let list = view.word1().map_err(|_| ModelError::DmaSubmissionState)?;
    let opcode = view.opcode().map_err(|_| ModelError::DmaSubmissionState)?;
    let (fault, order) = match error {
        DescriptorDecodeError::InvalidVersion { observed, .. } => (
            AbiFault::invalid_version(slot, observed),
            AbiFaultOrder::Header(AbiHeaderField::Version),
        ),
        DescriptorDecodeError::InvalidSize { observed, .. } => (
            AbiFault::invalid_size(slot, observed),
            AbiFaultOrder::Header(AbiHeaderField::Size),
        ),
        DescriptorDecodeError::InvalidOpcode { observed, .. } => (
            AbiFault::invalid_opcode(slot, observed),
            AbiFaultOrder::Header(AbiHeaderField::Opcode),
        ),
        DescriptorDecodeError::InvalidFlags { observed, .. } => (
            AbiFault::invalid_flags(slot, observed),
            AbiFaultOrder::Header(AbiHeaderField::Flags),
        ),
        DescriptorDecodeError::InvalidReserved { offset, observed } => (
            AbiFault::invalid_reserved(slot, u64::from(offset), observed),
            AbiFaultOrder::Reserved(reserved(opcode, offset)?),
        ),
        DescriptorDecodeError::ValueOutOfRange {
            offset, observed, ..
        } => (
            AbiFault::value_out_of_range(slot, u64::from(offset), observed),
            AbiFaultOrder::Scalar(scalar(opcode, offset)?),
        ),
        DescriptorDecodeError::InvalidIndirectCount { observed, .. } => (
            AbiFault::invalid_indirect_count(DmaAddress::new(list), observed),
            AbiFaultOrder::Scalar(AbiScalarField::BroadcastDestinationCount),
        ),
        DescriptorDecodeError::InvalidIndirectAlignment { address, .. } => (
            AbiFault::invalid_indirect_alignment(DmaAddress::new(address)),
            AbiFaultOrder::Scalar(AbiScalarField::BroadcastListAlignment),
        ),
        DescriptorDecodeError::Codec(_) | DescriptorDecodeError::MetadataMismatch => {
            return Err(ModelError::DmaSubmissionState);
        }
    };
    Ok(canonical(fault, order))
}

pub(super) fn canonical(fault: AbiFault, order: AbiFaultOrder) -> AbiFault {
    let candidate = AbiFaultCandidate::try_new(fault, order);
    match candidate.and_then(|candidate| select_canonical_fault([candidate])) {
        Ok(Some(selected)) => selected,
        Ok(None) | Err(_) => fault,
    }
}

fn reserved(opcode: u16, offset: u16) -> ModelResult<AbiReservedField> {
    match (opcode, usize::from(offset)) {
        (COPY_OPCODE, DescriptorSchema::WORD3_OFFSET) => Ok(AbiReservedField::CopyOffset40),
        (COPY_OPCODE, DescriptorSchema::WORD4_OFFSET) => Ok(AbiReservedField::CopyOffset48),
        (COPY_OPCODE, DescriptorSchema::WORD5_OFFSET) => Ok(AbiReservedField::CopyOffset56),
        (BROADCAST_COPY_OPCODE, DescriptorSchema::WORD4_OFFSET) => {
            Ok(AbiReservedField::BroadcastOffset48)
        }
        (BROADCAST_COPY_OPCODE, DescriptorSchema::WORD5_OFFSET) => {
            Ok(AbiReservedField::BroadcastOffset56)
        }
        (VECTOR_ADD_OPCODE, DescriptorSchema::WORD4_OFFSET) => Ok(AbiReservedField::VectorOffset48),
        (VECTOR_ADD_OPCODE, DescriptorSchema::WORD5_OFFSET) => Ok(AbiReservedField::VectorOffset56),
        _ => Err(ModelError::DmaSubmissionState),
    }
}

fn scalar(opcode: u16, offset: u16) -> ModelResult<AbiScalarField> {
    match (opcode, usize::from(offset)) {
        (COPY_OPCODE, DescriptorSchema::WORD2_OFFSET) => Ok(AbiScalarField::CopyLength),
        (STRIDED_COPY_OPCODE, DescriptorSchema::WORD2_OFFSET) => Ok(AbiScalarField::StridedRows),
        (STRIDED_COPY_OPCODE, DescriptorSchema::WORD3_OFFSET) => {
            Ok(AbiScalarField::StridedRowBytes)
        }
        (STRIDED_COPY_OPCODE, DescriptorSchema::WORD4_OFFSET) => {
            Ok(AbiScalarField::StridedSourceStride)
        }
        (STRIDED_COPY_OPCODE, DescriptorSchema::WORD5_OFFSET) => {
            Ok(AbiScalarField::StridedDestinationStride)
        }
        (BROADCAST_COPY_OPCODE, DescriptorSchema::WORD2_OFFSET) => {
            Ok(AbiScalarField::BroadcastLength)
        }
        (VECTOR_ADD_OPCODE, DescriptorSchema::WORD3_OFFSET) => Ok(AbiScalarField::VectorLength),
        _ => Err(ModelError::DmaSubmissionState),
    }
}
