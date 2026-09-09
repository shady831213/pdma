use pdma_model_contract::abi::AbiEventKind;
use pdma_model_contract::abi::{AbiCompletion, AbiCompletionStatus};

pub(super) const fn completion_detail(record: AbiCompletion) -> &'static str {
    match record.status {
        AbiCompletionStatus::Success => "status Success",
    }
}

pub(super) const fn event_detail(error: AbiEventKind) -> &'static str {
    match error {
        AbiEventKind::UnmappedDmaAddress => "Error UnknownBuffer",
        AbiEventKind::RangeOverflow => "Error RangeOverflow",
        AbiEventKind::OutOfBounds => "Error OutOfBounds",
        AbiEventKind::InvalidVersion
        | AbiEventKind::InvalidSize
        | AbiEventKind::InvalidOpcode
        | AbiEventKind::InvalidFlags
        | AbiEventKind::InvalidReserved
        | AbiEventKind::InvalidIndirectCount
        | AbiEventKind::InvalidIndirectAlignment
        | AbiEventKind::ValueOutOfRange => "Error InvalidDescriptor",
    }
}
