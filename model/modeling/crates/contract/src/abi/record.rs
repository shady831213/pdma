//! Fixed-width completion and event record codecs.

mod fault;
mod value;

pub use fault::{
    AbiFault, AbiFaultCandidate, AbiFaultOrder, AbiFaultSelectionError, AbiHeaderField,
    AbiOperandFaultStep, AbiOperandVisit, AbiReservedField, AbiScalarField, select_canonical_fault,
};
pub use value::{
    AbiBounds, AbiEventAccess, AbiEventKind, AbiEventStage, AbiFaultDetails, AbiFaultSite,
    AbiStrideOffset,
};

use super::{
    ABI_VERSION, AbiDecodeError, AbiEncodeError, AbiId, COMPLETION_SIZE, CompletionSchema,
    CompletionSchemaView, CompletionSchemaViewMut, EVENT_SIZE, EventSchema, EventSchemaView,
    EventSchemaViewMut, SUCCESS_STATUS,
    codec::{decode_schema_error, encode_schema_error},
};

/// Exact ABI-v1 successful completion value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiCompletion {
    queue_id: AbiId,
    command_id: AbiId,
}

impl AbiCompletion {
    /// Creates an exact completion value.
    pub const fn new(queue_id: AbiId, command_id: AbiId) -> Self {
        Self {
            queue_id,
            command_id,
        }
    }

    /// Returns the fixed-width queue identifier.
    pub const fn queue_id(self) -> AbiId {
        self.queue_id
    }

    /// Returns the fixed-width command identifier.
    pub const fn command_id(self) -> AbiId {
        self.command_id
    }
}

/// Address-aware event value used by the DMA ABI path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiEvent {
    queue_id: AbiId,
    command_id: AbiId,
    fault: AbiFault,
}

impl AbiEvent {
    /// Creates one event after runtime first-match classification.
    pub const fn new(queue_id: AbiId, command_id: AbiId, fault: AbiFault) -> Self {
        Self {
            queue_id,
            command_id,
            fault,
        }
    }

    /// Returns the submission-ring queue identity.
    pub const fn queue_id(self) -> AbiId {
        self.queue_id
    }

    /// Returns the consumed descriptor command identity.
    pub const fn command_id(self) -> AbiId {
        self.command_id
    }

    /// Returns the canonical fault tuple.
    pub const fn fault(self) -> AbiFault {
        self.fault
    }
}

/// Encodes one successful completion into exactly 32 ABI-v1 bytes.
pub fn encode_completion(
    record: AbiCompletion,
) -> Result<[u8; CompletionSchema::SPEC.size()], AbiEncodeError> {
    let mut bytes = [0_u8; CompletionSchema::SPEC.size()];
    let mut view = CompletionSchemaViewMut::new(&mut bytes).map_err(encode_schema_error)?;
    view.set_version(ABI_VERSION).map_err(encode_schema_error)?;
    view.set_size(COMPLETION_SIZE)
        .map_err(encode_schema_error)?;
    view.set_status(SUCCESS_STATUS)
        .map_err(encode_schema_error)?;
    view.set_flags(0).map_err(encode_schema_error)?;
    view.set_origin_queue_id(record.queue_id().raw())
        .map_err(encode_schema_error)?;
    view.set_command_id(record.command_id().raw())
        .map_err(encode_schema_error)?;
    view.set_reserved(0).map_err(encode_schema_error)?;
    Ok(bytes)
}

/// Decodes and validates one ABI-v1 success completion.
pub fn decode_completion(bytes: &[u8]) -> Result<AbiCompletion, AbiDecodeError> {
    let view = CompletionSchemaView::new(bytes).map_err(decode_schema_error)?;
    require_field(
        "version",
        u64::from(view.version().map_err(decode_schema_error)?),
        u64::from(ABI_VERSION),
    )?;
    require_field(
        "size",
        u64::from(view.size().map_err(decode_schema_error)?),
        u64::from(COMPLETION_SIZE),
    )?;
    let status = view.status().map_err(decode_schema_error)?;
    if status != SUCCESS_STATUS {
        return Err(AbiDecodeError::UnknownCompletionStatus { value: status });
    }
    require_field(
        "flags",
        u64::from(view.flags().map_err(decode_schema_error)?),
        0,
    )?;
    let queue_id = AbiId::new(view.origin_queue_id().map_err(decode_schema_error)?);
    let command_id = AbiId::new(view.command_id().map_err(decode_schema_error)?);
    require_field("reserved", view.reserved().map_err(decode_schema_error)?, 0)?;
    Ok(AbiCompletion::new(queue_id, command_id))
}

/// Encodes one canonical address-aware event into exactly 64 ABI-v1 bytes.
pub fn encode_event(event: AbiEvent) -> Result<[u8; EventSchema::SPEC.size()], AbiEncodeError> {
    if !event.fault.is_canonical() {
        return Err(AbiEncodeError::NonCanonicalEventDetails);
    }
    let mut bytes = [0_u8; EventSchema::SPEC.size()];
    let tuple = event.fault.tuple();
    let mut view = EventSchemaViewMut::new(&mut bytes).map_err(encode_schema_error)?;
    view.set_version(ABI_VERSION).map_err(encode_schema_error)?;
    view.set_size(EVENT_SIZE).map_err(encode_schema_error)?;
    view.set_kind(tuple.0.value())
        .map_err(encode_schema_error)?;
    view.set_flags(0).map_err(encode_schema_error)?;
    view.set_origin_queue_id(event.queue_id.raw())
        .map_err(encode_schema_error)?;
    view.set_command_id(event.command_id.raw())
        .map_err(encode_schema_error)?;
    view.set_fault_locator(tuple.1)
        .map_err(encode_schema_error)?;
    view.set_detail0(tuple.2).map_err(encode_schema_error)?;
    view.set_detail1(tuple.3).map_err(encode_schema_error)?;
    view.set_stage(tuple.4).map_err(encode_schema_error)?;
    view.set_access(tuple.5).map_err(encode_schema_error)?;
    view.set_reserved(0).map_err(encode_schema_error)?;
    view.set_index(tuple.6).map_err(encode_schema_error)?;
    Ok(bytes)
}

/// Decodes and validates one canonical address-aware ABI-v1 event.
pub fn decode_event(bytes: &[u8]) -> Result<AbiEvent, AbiDecodeError> {
    let view = EventSchemaView::new(bytes).map_err(decode_schema_error)?;
    require_field(
        "version",
        u64::from(view.version().map_err(decode_schema_error)?),
        u64::from(ABI_VERSION),
    )?;
    require_field(
        "size",
        u64::from(view.size().map_err(decode_schema_error)?),
        u64::from(EVENT_SIZE),
    )?;
    let kind = AbiEventKind::parse(view.kind().map_err(decode_schema_error)?)?;
    require_field(
        "flags",
        u64::from(view.flags().map_err(decode_schema_error)?),
        0,
    )?;
    let queue_id = AbiId::new(view.origin_queue_id().map_err(decode_schema_error)?);
    let command_id = AbiId::new(view.command_id().map_err(decode_schema_error)?);
    let locator = super::DmaAddress::new(view.fault_locator().map_err(decode_schema_error)?);
    let detail0 = view.detail0().map_err(decode_schema_error)?;
    let detail1 = view.detail1().map_err(decode_schema_error)?;
    let stage = AbiEventStage::parse(view.stage().map_err(decode_schema_error)?)?;
    let access = AbiEventAccess::parse(view.access().map_err(decode_schema_error)?)?;
    require_field(
        "reserved",
        u64::from(view.reserved().map_err(decode_schema_error)?),
        0,
    )?;
    let index = view.index().map_err(decode_schema_error)?;
    let fault = AbiFault::from_parts(
        kind,
        locator,
        AbiFaultDetails::new(detail0, detail1, AbiFaultSite::new(stage, access, index)),
    );
    if !fault.is_canonical() {
        return Err(AbiDecodeError::NonCanonicalEventDetails);
    }
    Ok(AbiEvent::new(queue_id, command_id, fault))
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
