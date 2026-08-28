mod completion;
mod event;

use crate::abi::{AbiDecodeError, AbiEventAccess, AbiEventStage};

const fn require_field(
    field: &'static str,
    value: u64,
    expected: u64,
) -> Result<(), AbiDecodeError> {
    if value == expected {
        Ok(())
    } else {
        Err(AbiDecodeError::InvalidRecordField { field, value })
    }
}

fn event_stage(value: u16) -> Result<AbiEventStage, AbiDecodeError> {
    AbiEventStage::from_raw(value).map_err(|_| AbiDecodeError::UnknownEventStage { value })
}

fn event_access(value: u16) -> Result<AbiEventAccess, AbiDecodeError> {
    AbiEventAccess::from_raw(value).map_err(|_| AbiDecodeError::UnknownEventAccess { value })
}
