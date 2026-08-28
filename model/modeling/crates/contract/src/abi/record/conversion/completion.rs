use crate::abi::{
    ABI_VERSION, AbiCompletion, AbiCompletionFlagsValues, AbiCompletionReservedValues,
    AbiCompletionStatus, AbiCompletionView, AbiDecodeError, AbiEncodeError, COMPLETION_SIZE,
    codec::{decode_schema_error, encode_schema_error},
};
use crate::semantic::{CommandId, CompletionRecord, CompletionStatus, QueueId};

use super::require_field;

impl AbiCompletion {
    /// Creates one canonical successful completion.
    pub const fn success(origin_queue_id: u64, command_id: u64) -> Self {
        Self::new(
            ABI_VERSION,
            AbiCompletionStatus::Success,
            AbiCompletionFlagsValues::ZERO,
            origin_queue_id,
            command_id,
            AbiCompletionReservedValues::ZERO,
        )
    }

    /// Encodes one successful completion into caller-owned ABI storage.
    ///
    /// # Errors
    /// Returns a typed error when the value is not ABI v1 or storage is too short.
    pub fn encode(self, bytes: &mut [u8]) -> Result<(), AbiEncodeError> {
        if self.version != ABI_VERSION {
            return Err(AbiEncodeError::InvalidRecordMetadata);
        }
        let actual = bytes.len();
        let destination =
            bytes
                .get_mut(..Self::RECORD_SIZE)
                .ok_or(AbiEncodeError::BufferTooShort {
                    required: Self::RECORD_SIZE,
                    actual,
                })?;
        let encoded = self.to_bytes().map_err(encode_schema_error)?;
        destination.copy_from_slice(&encoded);
        Ok(())
    }

    /// Decodes and validates one ABI-v1 successful completion.
    ///
    /// # Errors
    /// Returns the first fixed-field or structural ABI error.
    pub fn decode(bytes: &[u8]) -> Result<Self, AbiDecodeError> {
        let view = AbiCompletionView::new(bytes).map_err(decode_schema_error)?;
        let version = view.version().map_err(decode_schema_error)?;
        require_field("version", u64::from(version), u64::from(ABI_VERSION))?;
        let size = view.size().map_err(decode_schema_error)?;
        require_field("size", u64::from(size), u64::from(COMPLETION_SIZE))?;
        let status = view.status().map_err(decode_schema_error)?;
        AbiCompletionStatus::from_raw(status)
            .map_err(|_| AbiDecodeError::UnknownCompletionStatus { value: status })?;
        let flags = view.flags().map_err(decode_schema_error)?;
        require_field("flags", u64::from(flags), 0)?;
        let reserved = view.reserved().map_err(decode_schema_error)?;
        require_field("reserved", reserved, 0)?;
        Self::from_bytes(bytes).map_err(decode_schema_error)
    }
}

impl TryFrom<CompletionRecord> for AbiCompletion {
    type Error = AbiEncodeError;

    fn try_from(record: CompletionRecord) -> Result<Self, Self::Error> {
        match record.status() {
            CompletionStatus::Success => Ok(Self::success(
                encode_model_id(record.queue_id().raw())?,
                encode_model_id(record.command_id().raw())?,
            )),
        }
    }
}

impl TryFrom<AbiCompletion> for CompletionRecord {
    type Error = AbiDecodeError;

    fn try_from(completion: AbiCompletion) -> Result<Self, Self::Error> {
        Ok(Self::new(
            QueueId::new(decode_model_id(completion.origin_queue_id)?),
            CommandId::new(decode_model_id(completion.command_id)?),
            CompletionStatus::Success,
        ))
    }
}

fn encode_model_id(value: usize) -> Result<u64, AbiEncodeError> {
    u64::try_from(value).map_err(|_| AbiEncodeError::UnrepresentableModelId { value })
}

fn decode_model_id(value: u64) -> Result<usize, AbiDecodeError> {
    usize::try_from(value).map_err(|_| AbiDecodeError::UnrepresentableId { value })
}
