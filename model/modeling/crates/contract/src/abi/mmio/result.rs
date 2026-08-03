macro_rules! numeric_enum {
    ($name:ident, $error:ident, $doc:literal, [$(($variant:ident,$code:literal,$variant_doc:literal)),+ $(,)?]) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name { $(#[doc = "Assigned command encoding."] $variant),+ }
        impl $name {
            /// Returns the stable numeric encoding.
            pub const fn code(self) -> u32 { match self { $(Self::$variant => $code),+ } }
        }
        impl TryFrom<u32> for $name {
            type Error = $error;
            fn try_from(value: u32) -> Result<Self, Self::Error> {
                match value { $($code => Ok(Self::$variant),)+ raw => Err($error(raw)) }
            }
        }
    };
}

/// An unassigned command encoding retained without truncation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownCommand(pub u32);

numeric_enum!(
    GlobalCommand,
    UnknownCommand,
    "Global command encoding.",
    [
        (Discover, 1, "DISCOVER"),
        (ReadLimits, 2, "READ_LIMITS"),
        (ReadStatus, 3, "READ_STATUS"),
        (Reset, 4, "RESET")
    ]
);
numeric_enum!(
    QueueCommand,
    UnknownCommand,
    "Queue command encoding.",
    [
        (Activate, 1, "ACTIVATE"),
        (ReadStatus, 2, "READ_STATUS"),
        (PublishSqProducer, 3, "PUBLISH_SQ_PRODUCER"),
        (CaptureSqProducer, 4, "CAPTURE_SQ_PRODUCER"),
        (PublishSqConsumer, 5, "PUBLISH_SQ_CONSUMER"),
        (PublishCqProducer, 6, "PUBLISH_CQ_PRODUCER"),
        (PublishCqConsumer, 7, "PUBLISH_CQ_CONSUMER"),
        (PublishEqProducer, 8, "PUBLISH_EQ_PRODUCER"),
        (PublishEqConsumer, 9, "PUBLISH_EQ_CONSUMER")
    ]
);
numeric_enum!(
    IrqCommand,
    UnknownCommand,
    "IRQ command encoding.",
    [
        (ConfigureRoute, 1, "CONFIGURE_ROUTE"),
        (Enable, 2, "ENABLE"),
        (Disable, 3, "DISABLE"),
        (Mask, 4, "MASK"),
        (Unmask, 5, "UNMASK"),
        (Acknowledge, 6, "ACKNOWLEDGE"),
        (ObserveSource, 7, "OBSERVE_SOURCE"),
        (SnapshotLine, 8, "SNAPSHOT_LINE"),
        (ClaimNotification, 9, "CLAIM_NOTIFICATION")
    ]
);

macro_rules! result_registry {
    ($(($variant:ident,$code:literal,$doc:literal)),+ $(,)?) => {
        /// Common command-result encoding shared by all three MMIO blocks.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum MmioResult { $(#[doc = "Assigned result encoding."] $variant),+ }
        impl MmioResult {
            /// Every assigned result in numeric order.
            pub const ALL: [Self; 32] = [$(Self::$variant),+];
            /// Returns the stable numeric result code.
            pub const fn code(self) -> u32 { match self { $(Self::$variant => $code),+ } }
            /// Reports whether this code represents an error.
            pub const fn is_error(self) -> bool { self.code() >= 0x100 }
        }
        impl TryFrom<u32> for MmioResult {
            type Error = UnknownResult;
            fn try_from(value: u32) -> Result<Self, Self::Error> {
                match value { $($code => Ok(Self::$variant),)+ raw => Err(UnknownResult(raw)) }
            }
        }
    };
}

/// An unassigned command-result encoding retained without truncation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownResult(pub u32);

result_registry!(
    (NotExecuted, 0, "NOT_EXECUTED"),
    (Ok, 1, "OK"),
    (Empty, 2, "EMPTY"),
    (UnsupportedVersion, 0x100, "UNSUPPORTED_VERSION"),
    (UnknownFeatureBits, 0x101, "UNKNOWN_FEATURE_BITS"),
    (StaleEpoch, 0x102, "STALE_EPOCH"),
    (InvalidQueue, 0x103, "INVALID_QUEUE"),
    (InvalidIrqSource, 0x104, "INVALID_IRQ_SOURCE"),
    (IrqSourceActive, 0x105, "IRQ_SOURCE_ACTIVE"),
    (InvalidRingDepth, 0x106, "INVALID_RING_DEPTH"),
    (RingExtentOverflow, 0x107, "RING_EXTENT_OVERFLOW"),
    (MisalignedRing, 0x108, "MISALIGNED_RING"),
    (RingExtentMismatch, 0x109, "RING_EXTENT_MISMATCH"),
    (UnexpectedRingRecord, 0x10a, "UNEXPECTED_RING_RECORD"),
    (QueueDepthMismatch, 0x10b, "QUEUE_DEPTH_MISMATCH"),
    (EventDepthMismatch, 0x10c, "EVENT_DEPTH_MISMATCH"),
    (CursorRegression, 0x10d, "CURSOR_REGRESSION"),
    (CursorUpdateRegression, 0x10e, "CURSOR_UPDATE_REGRESSION"),
    (CursorDistanceExceeded, 0x10f, "CURSOR_DISTANCE_EXCEEDED"),
    (CursorIncrementOverflow, 0x110, "CURSOR_INCREMENT_OVERFLOW"),
    (DeviceEpochOverflow, 0x111, "DEVICE_EPOCH_OVERFLOW"),
    (UnknownStatusValue, 0x112, "UNKNOWN_STATUS_VALUE"),
    (DmaAddressExhausted, 0x113, "DMA_ADDRESS_EXHAUSTED"),
    (DmaStorageExhausted, 0x114, "DMA_STORAGE_EXHAUSTED"),
    (DmaUnmapped, 0x115, "DMA_UNMAPPED"),
    (DmaCrossesRegion, 0x116, "DMA_CROSSES_REGION"),
    (DmaInvalidRegion, 0x117, "DMA_INVALID_REGION"),
    (DmaOverlap, 0x118, "DMA_OVERLAP"),
    (DmaStaleStage, 0x119, "DMA_STALE_STAGE"),
    (InvalidTransaction, 0x11a, "INVALID_TRANSACTION"),
    (UnknownCommand, 0x11b, "UNKNOWN_COMMAND"),
    (UnknownEncoding, 0x11c, "UNKNOWN_ENCODING"),
);

/// One result latch and its exact three details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MmioResultDetails {
    /// Common result code.
    pub result: MmioResult,
    /// Result-specific detail words.
    pub details: [u64; 3],
}

/// Atomic effect of a structurally valid command on its response bank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseUpdate<T> {
    /// A successful response replaces every field.
    Replace(T),
    /// An EMPTY result replaces every field with zero/default.
    Empty,
    /// A semantic failure preserves the prior successful response.
    Preserve,
}

/// Applies the replacement, EMPTY zero-fill, or failure-preservation rule.
pub fn apply_response_update<T: Copy + Default>(previous: T, update: ResponseUpdate<T>) -> T {
    match update {
        ResponseUpdate::Replace(response) => response,
        ResponseUpdate::Empty => T::default(),
        ResponseUpdate::Preserve => previous,
    }
}
