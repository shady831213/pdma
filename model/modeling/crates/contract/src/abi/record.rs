//! Fixed-width completion and event record codecs.

mod conversion;
mod schema;

pub use schema::{
    CompletionSchema, CompletionSchemaRaw, CompletionSchemaValues, CompletionSchemaView,
    CompletionSchemaViewMut, EventSchema, EventSchemaRaw, EventSchemaValues, EventSchemaView,
    EventSchemaViewMut,
};

const _: () = assert!(EventSchema::RECORD_SIZE == 64);

use super::{AbiDecodeError, AbiEncodeError, AbiId, fault::AbiFault};

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
pub struct ExactAbiEvent {
    queue_id: AbiId,
    command_id: AbiId,
    fault: AbiFault,
}

impl ExactAbiEvent {
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
    CompletionSchemaRaw::try_from(record).map(CompletionSchemaRaw::into_bytes)
}

/// Decodes and validates one ABI-v1 success completion.
pub fn decode_completion(bytes: &[u8]) -> Result<AbiCompletion, AbiDecodeError> {
    let actual = bytes.len();
    let source =
        bytes
            .get(..CompletionSchema::RECORD_SIZE)
            .ok_or(AbiDecodeError::BufferTooShort {
                required: CompletionSchema::RECORD_SIZE,
                actual,
            })?;
    let mut raw = [0_u8; CompletionSchema::RECORD_SIZE];
    raw.copy_from_slice(source);
    AbiCompletion::try_from(CompletionSchemaRaw::new(raw))
}

/// Encodes one canonical address-aware event into exactly 64 ABI-v1 bytes.
pub fn encode_event(
    event: ExactAbiEvent,
) -> Result<[u8; EventSchema::SPEC.size()], AbiEncodeError> {
    EventSchemaRaw::try_from(event).map(EventSchemaRaw::into_bytes)
}

/// Decodes and validates one canonical address-aware ABI-v1 event.
pub fn decode_event(bytes: &[u8]) -> Result<ExactAbiEvent, AbiDecodeError> {
    let actual = bytes.len();
    let source = bytes
        .get(..EventSchema::RECORD_SIZE)
        .ok_or(AbiDecodeError::BufferTooShort {
            required: EventSchema::RECORD_SIZE,
            actual,
        })?;
    let mut raw = [0_u8; EventSchema::RECORD_SIZE];
    raw.copy_from_slice(source);
    ExactAbiEvent::try_from(EventSchemaRaw::new(raw))
}
