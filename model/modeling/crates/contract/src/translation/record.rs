//! Semantic adapters for exact ABI completion and event records.

use crate::abi::{
    AbiCompletion, AbiDecodeError, AbiEncodeError, AbiId, ExactAbiEvent, decode_exact_completion,
    decode_exact_event, encode_exact_completion, encode_exact_event,
};
use crate::binding::AbiEvent;
use crate::semantic::{CommandId, CompletionRecord, CompletionStatus, QueueId};

/// Encodes one semantic successful completion.
pub fn encode_completion(record: CompletionRecord) -> Result<[u8; 32], AbiEncodeError> {
    let queue = AbiId::try_from(record.queue_id().raw())?;
    let command = AbiId::try_from(record.command_id().raw())?;
    encode_exact_completion(AbiCompletion::new(queue, command))
}

/// Decodes one exact successful completion into semantic identities.
pub fn decode_completion(bytes: &[u8]) -> Result<CompletionRecord, AbiDecodeError> {
    let exact = decode_exact_completion(bytes)?;
    Ok(CompletionRecord::new(
        QueueId::new(exact.queue_id().try_into_model()?),
        CommandId::new(exact.command_id().try_into_model()?),
        CompletionStatus::Success,
    ))
}

/// Encodes one semantic-identity, address-aware event.
pub fn encode_event(event: AbiEvent) -> Result<[u8; 64], AbiEncodeError> {
    let queue = AbiId::try_from(event.queue_id().raw())?;
    let command = AbiId::try_from(event.command_id().raw())?;
    encode_exact_event(ExactAbiEvent::new(queue, command, event.fault()))
}

/// Decodes one exact event into semantic model identities.
pub fn decode_event(bytes: &[u8]) -> Result<AbiEvent, AbiDecodeError> {
    let exact = decode_exact_event(bytes)?;
    Ok(AbiEvent::new(
        QueueId::new(exact.queue_id().try_into_model()?),
        CommandId::new(exact.command_id().try_into_model()?),
        exact.fault(),
    ))
}
