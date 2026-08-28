use crate::abi::{
    ABI_VERSION, AbiDecodeError, AbiEncodeError, AbiEvent, AbiEventFlagsValues, AbiEventKind,
    AbiEventReservedValues, AbiEventView, AbiFault, AbiFaultDetails, AbiFaultSite, DmaAddress,
    EVENT_SIZE,
    codec::{decode_schema_error, encode_schema_error},
};

use super::require_field;

impl AbiEvent {
    /// Creates one ABI event from its canonical fault vocabulary.
    pub const fn new(origin_queue_id: u64, command_id: u64, fault: AbiFault) -> Self {
        let tuple = fault.tuple();
        Self {
            version: ABI_VERSION,
            kind: tuple.0,
            flags: AbiEventFlagsValues::ZERO,
            origin_queue_id,
            command_id,
            fault_locator: tuple.1,
            detail0: tuple.2,
            detail1: tuple.3,
            stage: fault.stage(),
            access: fault.access(),
            reserved: AbiEventReservedValues::ZERO,
            index: tuple.6,
        }
    }

    /// Reconstructs the canonical fault vocabulary carried by this record.
    pub const fn fault(self) -> AbiFault {
        AbiFault::from_parts(
            self.kind,
            DmaAddress::new(self.fault_locator),
            AbiFaultDetails::new(
                self.detail0,
                self.detail1,
                AbiFaultSite::new(self.stage, self.access, self.index),
            ),
        )
    }

    /// Encodes one canonical event into caller-owned ABI storage.
    ///
    /// # Errors
    /// Returns a typed error for noncanonical details, metadata, or short storage.
    pub fn encode(self, bytes: &mut [u8]) -> Result<(), AbiEncodeError> {
        if self.version != ABI_VERSION {
            return Err(AbiEncodeError::InvalidRecordMetadata);
        }
        if !self.fault().is_canonical() {
            return Err(AbiEncodeError::NonCanonicalEventDetails);
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

    /// Decodes and validates one canonical ABI-v1 event.
    ///
    /// # Errors
    /// Returns the first fixed-field, enum, or canonicality error.
    pub fn decode(bytes: &[u8]) -> Result<Self, AbiDecodeError> {
        let view = AbiEventView::new(bytes).map_err(decode_schema_error)?;
        let version = view.version().map_err(decode_schema_error)?;
        require_field("version", u64::from(version), u64::from(ABI_VERSION))?;
        let size = view.size().map_err(decode_schema_error)?;
        require_field("size", u64::from(size), u64::from(EVENT_SIZE))?;
        let kind = view.kind().map_err(decode_schema_error)?;
        AbiEventKind::from_raw(kind)
            .map_err(|_| AbiDecodeError::UnknownEventKind { value: kind })?;
        let flags = view.flags().map_err(decode_schema_error)?;
        require_field("flags", u64::from(flags), 0)?;
        let stage = view.stage().map_err(decode_schema_error)?;
        super::event_stage(stage)?;
        let access = view.access().map_err(decode_schema_error)?;
        super::event_access(access)?;
        let reserved = view.reserved().map_err(decode_schema_error)?;
        require_field("reserved", u64::from(reserved), 0)?;
        let values = Self::from_bytes(bytes).map_err(decode_schema_error)?;
        let fault = values.fault();
        if !fault.is_canonical() {
            return Err(AbiDecodeError::NonCanonicalEventDetails);
        }
        Ok(values)
    }
}
