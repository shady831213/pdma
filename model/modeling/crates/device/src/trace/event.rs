use pdma_model_contract::semantic::completion::{CompletionRecord, CompletionStatus};
use pdma_model_contract::semantic::event::{EventErrorKind, EventRecord};

pub(super) const fn completion_detail(record: CompletionRecord) -> &'static str {
    match record.status() {
        CompletionStatus::Success => "status Success",
    }
}

pub(super) const fn event_detail(record: EventRecord) -> &'static str {
    match record.error_kind() {
        EventErrorKind::UnknownBuffer => "Error UnknownBuffer",
        EventErrorKind::RangeOverflow => "Error RangeOverflow",
        EventErrorKind::OutOfBounds { len: _, end: _ } => "Error OutOfBounds",
    }
}
