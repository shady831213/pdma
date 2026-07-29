use pdma_model_contract::abi::{
    AbiFault, AbiFaultCandidate, AbiFaultOrder, AbiHeaderField, AbiReservedField, AbiScalarField,
    DescriptorDecodeError, DmaAddress, select_canonical_fault,
};
use pdma_model_contract::error::{ModelError, ModelResult};

pub(super) fn descriptor(
    slot: DmaAddress,
    bytes: &[u8],
    error: DescriptorDecodeError,
) -> ModelResult<AbiFault> {
    let list = read_u64(bytes, 24..32)?;
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
            AbiFaultOrder::Reserved(reserved(read_u16(bytes, 2..4)?, offset)?),
        ),
        DescriptorDecodeError::ValueOutOfRange {
            offset, observed, ..
        } => (
            AbiFault::value_out_of_range(slot, u64::from(offset), observed),
            AbiFaultOrder::Scalar(scalar(read_u16(bytes, 2..4)?, offset)?),
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

const fn reserved(opcode: u16, offset: u16) -> ModelResult<AbiReservedField> {
    match (opcode, offset) {
        (1, 40) => Ok(AbiReservedField::CopyOffset40),
        (1, 48) => Ok(AbiReservedField::CopyOffset48),
        (1, 56) => Ok(AbiReservedField::CopyOffset56),
        (3, 48) => Ok(AbiReservedField::BroadcastOffset48),
        (3, 56) => Ok(AbiReservedField::BroadcastOffset56),
        (4, 48) => Ok(AbiReservedField::VectorOffset48),
        (4, 56) => Ok(AbiReservedField::VectorOffset56),
        _ => Err(ModelError::DmaSubmissionState),
    }
}

const fn scalar(opcode: u16, offset: u16) -> ModelResult<AbiScalarField> {
    match (opcode, offset) {
        (1, 32) => Ok(AbiScalarField::CopyLength),
        (2, 32) => Ok(AbiScalarField::StridedRows),
        (2, 40) => Ok(AbiScalarField::StridedRowBytes),
        (2, 48) => Ok(AbiScalarField::StridedSourceStride),
        (2, 56) => Ok(AbiScalarField::StridedDestinationStride),
        (3, 32) => Ok(AbiScalarField::BroadcastLength),
        (4, 40) => Ok(AbiScalarField::VectorLength),
        _ => Err(ModelError::DmaSubmissionState),
    }
}

fn read_u16(bytes: &[u8], range: core::ops::Range<usize>) -> ModelResult<u16> {
    pdma_model_contract::abi::read_u16_le(bytes, range).map_err(|_| ModelError::DmaSubmissionState)
}

fn read_u64(bytes: &[u8], range: core::ops::Range<usize>) -> ModelResult<u64> {
    pdma_model_contract::abi::read_u64_le(bytes, range).map_err(|_| ModelError::DmaSubmissionState)
}
