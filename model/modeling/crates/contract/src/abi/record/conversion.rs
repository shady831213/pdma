use crate::abi::{
    ABI_VERSION, AbiDecodeError, AbiEncodeError, AbiEventAccess, AbiEventKind, AbiEventStage,
    AbiFault, AbiFaultDetails, AbiFaultSite, AbiId, COMPLETION_SIZE, DmaAddress, EVENT_SIZE,
    SUCCESS_STATUS,
    codec::{decode_schema_error, encode_schema_error},
};
use crate::semantic::{CommandId, CompletionRecord, CompletionStatus, QueueId};

use super::{
    AbiCompletion, CompletionSchemaRaw, CompletionSchemaValues, EventSchemaRaw, EventSchemaValues,
    ExactAbiEvent,
};

impl TryFrom<CompletionSchemaRaw> for AbiCompletion {
    type Error = AbiDecodeError;

    fn try_from(raw: CompletionSchemaRaw) -> Result<Self, Self::Error> {
        let values = CompletionSchemaValues::try_from(raw).map_err(decode_schema_error)?;
        require_field("version", u64::from(values.version), u64::from(ABI_VERSION))?;
        require_field("size", u64::from(values.size), u64::from(COMPLETION_SIZE))?;
        if values.status != SUCCESS_STATUS {
            return Err(AbiDecodeError::UnknownCompletionStatus {
                value: values.status,
            });
        }
        require_field("flags", u64::from(values.flags), 0)?;
        let completion = Self::new(
            AbiId::new(values.origin_queue_id),
            AbiId::new(values.command_id),
        );
        require_field("reserved", values.reserved, 0)?;
        Ok(completion)
    }
}

impl TryFrom<AbiCompletion> for CompletionSchemaRaw {
    type Error = AbiEncodeError;

    fn try_from(completion: AbiCompletion) -> Result<Self, Self::Error> {
        Self::try_from(CompletionSchemaValues {
            version: ABI_VERSION,
            size: COMPLETION_SIZE,
            status: SUCCESS_STATUS,
            flags: 0,
            origin_queue_id: completion.queue_id().raw(),
            command_id: completion.command_id().raw(),
            reserved: 0,
        })
        .map_err(encode_schema_error)
    }
}

impl TryFrom<CompletionRecord> for AbiCompletion {
    type Error = AbiEncodeError;

    fn try_from(record: CompletionRecord) -> Result<Self, Self::Error> {
        match record.status() {
            CompletionStatus::Success => Ok(Self::new(
                AbiId::try_from(record.queue_id().raw())?,
                AbiId::try_from(record.command_id().raw())?,
            )),
        }
    }
}

impl TryFrom<AbiCompletion> for CompletionRecord {
    type Error = AbiDecodeError;

    fn try_from(completion: AbiCompletion) -> Result<Self, Self::Error> {
        Ok(Self::new(
            QueueId::new(completion.queue_id().try_into_model()?),
            CommandId::new(completion.command_id().try_into_model()?),
            CompletionStatus::Success,
        ))
    }
}

impl TryFrom<EventSchemaRaw> for ExactAbiEvent {
    type Error = AbiDecodeError;

    fn try_from(raw: EventSchemaRaw) -> Result<Self, Self::Error> {
        let values = EventSchemaValues::try_from(raw).map_err(decode_schema_error)?;
        require_field("version", u64::from(values.version), u64::from(ABI_VERSION))?;
        require_field("size", u64::from(values.size), u64::from(EVENT_SIZE))?;
        let kind = AbiEventKind::parse(values.kind)?;
        require_field("flags", u64::from(values.flags), 0)?;
        let stage = AbiEventStage::parse(values.stage)?;
        let access = AbiEventAccess::parse(values.access)?;
        require_field("reserved", u64::from(values.reserved), 0)?;
        let fault = AbiFault::from_parts(
            kind,
            DmaAddress::new(values.fault_locator),
            AbiFaultDetails::new(
                values.detail0,
                values.detail1,
                AbiFaultSite::new(stage, access, values.index),
            ),
        );
        if !fault.is_canonical() {
            return Err(AbiDecodeError::NonCanonicalEventDetails);
        }
        Ok(Self::new(
            AbiId::new(values.origin_queue_id),
            AbiId::new(values.command_id),
            fault,
        ))
    }
}

impl TryFrom<ExactAbiEvent> for EventSchemaRaw {
    type Error = AbiEncodeError;

    fn try_from(event: ExactAbiEvent) -> Result<Self, Self::Error> {
        if !event.fault().is_canonical() {
            return Err(AbiEncodeError::NonCanonicalEventDetails);
        }
        let tuple = event.fault().tuple();
        Self::try_from(EventSchemaValues {
            version: ABI_VERSION,
            size: EVENT_SIZE,
            kind: tuple.0.value(),
            flags: 0,
            origin_queue_id: event.queue_id().raw(),
            command_id: event.command_id().raw(),
            fault_locator: tuple.1,
            detail0: tuple.2,
            detail1: tuple.3,
            stage: tuple.4,
            access: tuple.5,
            reserved: 0,
            index: tuple.6,
        })
        .map_err(encode_schema_error)
    }
}

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
