use pdma_model_contract::abi::{AbiCompletion, AbiCompletionStatus};
use pdma_model_contract::semantic::event::EventErrorKind;

pub(super) const fn completion_detail(record: AbiCompletion) -> &'static str {
    match record.status {
        AbiCompletionStatus::Success => "status Success",
    }
}

pub(super) const fn event_detail(error: EventErrorKind) -> &'static str {
    match error {
        EventErrorKind::UnknownBuffer => "Error UnknownBuffer",
        EventErrorKind::RangeOverflow => "Error RangeOverflow",
        EventErrorKind::OutOfBounds { len: _, end: _ } => "Error OutOfBounds",
    }
}
