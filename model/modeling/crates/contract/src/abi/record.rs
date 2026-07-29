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

use crate::completion::{CompletionRecord, CompletionStatus};
use crate::event::EventRecord;
use crate::ids::{CommandId, QueueId};

use super::{
    ABI_VERSION, AbiDecodeError, AbiEncodeError, AbiId, COMPLETION, COMPLETION_SIZE, EVENT,
    EVENT_SIZE, SUCCESS_STATUS, read_u16_le, read_u64_le, write_u16_le, write_u64_le,
};

/// Address-aware event value used by the DMA ABI path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiEvent {
    queue_id: QueueId,
    command_id: CommandId,
    fault: AbiFault,
}

impl AbiEvent {
    /// Creates one event after runtime first-match classification.
    pub const fn new(queue_id: QueueId, command_id: CommandId, fault: AbiFault) -> Self {
        Self {
            queue_id,
            command_id,
            fault,
        }
    }

    /// Returns the submission-ring queue identity.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }

    /// Returns the consumed descriptor command identity.
    pub const fn command_id(self) -> CommandId {
        self.command_id
    }

    /// Returns the canonical fault tuple.
    pub const fn fault(self) -> AbiFault {
        self.fault
    }
}

/// Encodes one successful completion into exactly 32 ABI-v1 bytes.
pub fn encode_completion(record: CompletionRecord) -> Result<[u8; 32], AbiEncodeError> {
    let mut bytes = [0_u8; 32];
    let &[version, size, status, flags, queue, command, reserved] = COMPLETION.fields() else {
        return Err(AbiEncodeError::InvalidRecordMetadata);
    };
    write_u16_le(&mut bytes, version.range(), ABI_VERSION)?;
    write_u16_le(&mut bytes, size.range(), COMPLETION_SIZE)?;
    write_u16_le(
        &mut bytes,
        status.range(),
        match record.status() {
            CompletionStatus::Success => SUCCESS_STATUS,
        },
    )?;
    write_u16_le(&mut bytes, flags.range(), 0)?;
    write_u64_le(
        &mut bytes,
        queue.range(),
        AbiId::try_from(record.queue_id().raw())?.raw(),
    )?;
    write_u64_le(
        &mut bytes,
        command.range(),
        AbiId::try_from(record.command_id().raw())?.raw(),
    )?;
    write_u64_le(&mut bytes, reserved.range(), 0)?;
    Ok(bytes)
}

/// Decodes and validates one ABI-v1 success completion.
pub fn decode_completion(bytes: &[u8]) -> Result<CompletionRecord, AbiDecodeError> {
    let &[version, size, status_field, flags, queue, command, reserved] = COMPLETION.fields()
    else {
        return Err(AbiDecodeError::InvalidRecordMetadata);
    };
    require_field(
        "version",
        u64::from(read_u16_le(bytes, version.range())?),
        u64::from(ABI_VERSION),
    )?;
    require_field(
        "size",
        u64::from(read_u16_le(bytes, size.range())?),
        u64::from(COMPLETION_SIZE),
    )?;
    let status = read_u16_le(bytes, status_field.range())?;
    if status != SUCCESS_STATUS {
        return Err(AbiDecodeError::UnknownCompletionStatus { value: status });
    }
    require_field("flags", u64::from(read_u16_le(bytes, flags.range())?), 0)?;
    let queue_id = decode_queue(read_u64_le(bytes, queue.range())?)?;
    let command_id = decode_command(read_u64_le(bytes, command.range())?)?;
    require_field("reserved", read_u64_le(bytes, reserved.range())?, 0)?;
    Ok(CompletionRecord::new(
        queue_id,
        command_id,
        CompletionStatus::Success,
    ))
}

/// Encodes one canonical address-aware event into exactly 64 ABI-v1 bytes.
pub fn encode_event(event: AbiEvent) -> Result<[u8; 64], AbiEncodeError> {
    if !event.fault.is_canonical() {
        return Err(AbiEncodeError::NonCanonicalEventDetails);
    }
    let mut bytes = [0_u8; 64];
    let &[
        version,
        size,
        kind,
        flags,
        queue,
        command,
        locator,
        detail0,
        detail1,
        stage,
        access,
        _reserved,
        index,
    ] = EVENT.fields()
    else {
        return Err(AbiEncodeError::InvalidRecordMetadata);
    };
    let tuple = event.fault.tuple();
    write_u16_le(&mut bytes, version.range(), ABI_VERSION)?;
    write_u16_le(&mut bytes, size.range(), EVENT_SIZE)?;
    write_u16_le(&mut bytes, kind.range(), tuple.0.value())?;
    write_u16_le(&mut bytes, flags.range(), 0)?;
    write_u64_le(
        &mut bytes,
        queue.range(),
        AbiId::try_from(event.queue_id.raw())?.raw(),
    )?;
    write_u64_le(
        &mut bytes,
        command.range(),
        AbiId::try_from(event.command_id.raw())?.raw(),
    )?;
    write_u64_le(&mut bytes, locator.range(), tuple.1)?;
    write_u64_le(&mut bytes, detail0.range(), tuple.2)?;
    write_u64_le(&mut bytes, detail1.range(), tuple.3)?;
    write_u16_le(&mut bytes, stage.range(), tuple.4)?;
    write_u16_le(&mut bytes, access.range(), tuple.5)?;
    write_u64_le(&mut bytes, index.range(), tuple.6)?;
    Ok(bytes)
}

/// Rejects semantic model events until an adapter supplies an IOVA fault locator.
pub const fn encode_semantic_event(_event: EventRecord) -> Result<[u8; 64], AbiEncodeError> {
    Err(AbiEncodeError::MissingEventIova)
}

/// Decodes and validates one canonical address-aware ABI-v1 event.
pub fn decode_event(bytes: &[u8]) -> Result<AbiEvent, AbiDecodeError> {
    let &[
        version,
        size,
        kind_field,
        flags,
        queue,
        command,
        locator_field,
        detail0_field,
        detail1_field,
        stage_field,
        access_field,
        reserved,
        index_field,
    ] = EVENT.fields()
    else {
        return Err(AbiDecodeError::InvalidRecordMetadata);
    };
    require_field(
        "version",
        u64::from(read_u16_le(bytes, version.range())?),
        u64::from(ABI_VERSION),
    )?;
    require_field(
        "size",
        u64::from(read_u16_le(bytes, size.range())?),
        u64::from(EVENT_SIZE),
    )?;
    let kind = AbiEventKind::parse(read_u16_le(bytes, kind_field.range())?)?;
    require_field("flags", u64::from(read_u16_le(bytes, flags.range())?), 0)?;
    let queue_id = decode_queue(read_u64_le(bytes, queue.range())?)?;
    let command_id = decode_command(read_u64_le(bytes, command.range())?)?;
    let locator = super::DmaAddress::new(read_u64_le(bytes, locator_field.range())?);
    let detail0 = read_u64_le(bytes, detail0_field.range())?;
    let detail1 = read_u64_le(bytes, detail1_field.range())?;
    let stage = AbiEventStage::parse(read_u16_le(bytes, stage_field.range())?)?;
    let access = AbiEventAccess::parse(read_u16_le(bytes, access_field.range())?)?;
    require_field(
        "reserved",
        u64::from(super::codec::read_u32_le(bytes, reserved.range())?),
        0,
    )?;
    let index = read_u64_le(bytes, index_field.range())?;
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

fn decode_queue(value: u64) -> Result<QueueId, AbiDecodeError> {
    AbiId::new(value).try_into_model().map(QueueId::new)
}

fn decode_command(value: u64) -> Result<CommandId, AbiDecodeError> {
    AbiId::new(value).try_into_model().map(CommandId::new)
}
