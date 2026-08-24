//! Fixed-width ABI-v1 ring values.

use super::{
    AbiControlError, COMPLETION_ALIGNMENT, COMPLETION_SIZE, DESCRIPTOR_ALIGNMENT, DESCRIPTOR_SIZE,
    DmaAddress, EVENT_ALIGNMENT, EVENT_SIZE, MAX_RING_DEPTH,
};

/// Canonical ABI-v1 record carried by a queue ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingRecord {
    /// Host-produced command descriptor.
    Submission,
    /// Device-produced successful completion.
    Completion,
    /// Device-produced fault event.
    Event,
}

impl RingRecord {
    /// Canonical record width in bytes.
    pub const fn record_size(self) -> u16 {
        match self {
            Self::Submission => DESCRIPTOR_SIZE,
            Self::Completion => COMPLETION_SIZE,
            Self::Event => EVENT_SIZE,
        }
    }

    /// Required base-I/O-address alignment.
    pub const fn alignment(self) -> u64 {
        match self {
            Self::Submission => DESCRIPTOR_ALIGNMENT,
            Self::Completion => COMPLETION_ALIGNMENT,
            Self::Event => EVENT_ALIGNMENT,
        }
    }
}

/// One exact ABI-v1 ring address, extent, depth, and record kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingLayout {
    base: DmaAddress,
    length: u64,
    depth: u32,
    extent: u64,
    record: RingRecord,
}

impl RingLayout {
    /// Validates an exact ring address, length, depth, and record kind.
    pub fn try_new(
        base: DmaAddress,
        length: u64,
        depth: u64,
        record: RingRecord,
    ) -> Result<Self, AbiControlError> {
        let depth = u32::try_from(depth)
            .ok()
            .filter(|value| *value != 0)
            .ok_or(AbiControlError::InvalidRingDepth {
                depth,
                maximum: MAX_RING_DEPTH,
            })?;
        let extent = Self::checked_extent(u64::from(depth), record.record_size())?;
        if base.raw() % record.alignment() != 0 {
            return Err(AbiControlError::MisalignedRing {
                base: base.raw(),
                alignment: record.alignment(),
            });
        }
        if length != extent {
            return Err(AbiControlError::RingExtentMismatch {
                expected: extent,
                actual: length,
            });
        }
        Ok(Self {
            base,
            length,
            depth,
            extent,
            record,
        })
    }

    /// Multiplies depth by record width without wrapping.
    pub fn checked_extent(depth: u64, record_size: u16) -> Result<u64, AbiControlError> {
        depth
            .checked_mul(u64::from(record_size))
            .ok_or(AbiControlError::RingExtentOverflow { depth, record_size })
    }

    /// Exact base DMA address.
    pub const fn base(self) -> DmaAddress {
        self.base
    }

    /// Exact backing byte length.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Retained ring capacity.
    pub const fn depth(self) -> u32 {
        self.depth
    }

    /// Exact required byte extent.
    pub const fn extent(self) -> u64 {
        self.extent
    }

    /// Canonical record kind.
    pub const fn record(self) -> RingRecord {
        self.record
    }

    /// Physical slot selected by a monotonic shadow.
    pub fn slot(self, cursor: RingCursor) -> u64 {
        cursor.raw() % u64::from(self.depth)
    }
}

/// One fixed-width monotonic queue cursor shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RingCursor(u64);

impl RingCursor {
    /// Creates a cursor from its complete monotonic shadow.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the complete monotonic shadow.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Advances without wrapping.
    pub const fn checked_increment(self, increment: u64) -> Result<Self, AbiControlError> {
        match self.0.checked_add(increment) {
            Some(next) => Ok(Self(next)),
            None => Err(AbiControlError::CursorIncrementOverflow {
                cursor: self.0,
                increment,
            }),
        }
    }
}

/// A validated producer/consumer pair for one exact ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingCursors {
    layout: RingLayout,
    producer: RingCursor,
    consumer: RingCursor,
    distance: u64,
}

impl RingCursors {
    /// Validates monotonic ordering and retained-capacity distance.
    pub fn try_new(
        layout: RingLayout,
        producer: RingCursor,
        consumer: RingCursor,
    ) -> Result<Self, AbiControlError> {
        let Some(distance) = producer.raw().checked_sub(consumer.raw()) else {
            return Err(AbiControlError::CursorRegression {
                producer: producer.raw(),
                consumer: consumer.raw(),
            });
        };
        if distance > u64::from(layout.depth()) {
            return Err(AbiControlError::CursorDistanceExceeded {
                producer: producer.raw(),
                consumer: consumer.raw(),
                depth: layout.depth(),
            });
        }
        Ok(Self {
            layout,
            producer,
            consumer,
            distance,
        })
    }

    /// Applies a forward or idempotent producer publication.
    pub fn update_producer(self, proposed: RingCursor) -> Result<Self, AbiControlError> {
        if proposed.raw() < self.producer.raw() {
            return Err(AbiControlError::CursorUpdateRegression {
                current: self.producer.raw(),
                proposed: proposed.raw(),
            });
        }
        Self::try_new(self.layout, proposed, self.consumer)
    }

    /// Applies a forward or idempotent consumer publication.
    pub fn update_consumer(self, proposed: RingCursor) -> Result<Self, AbiControlError> {
        if proposed.raw() < self.consumer.raw() {
            return Err(AbiControlError::CursorUpdateRegression {
                current: self.consumer.raw(),
                proposed: proposed.raw(),
            });
        }
        Self::try_new(self.layout, self.producer, proposed)
    }

    /// Producer shadow.
    pub const fn producer(self) -> RingCursor {
        self.producer
    }

    /// Consumer shadow.
    pub const fn consumer(self) -> RingCursor {
        self.consumer
    }

    /// Whether every retained slot is occupied.
    pub fn is_full(self) -> bool {
        self.distance == u64::from(self.layout.depth())
    }

    /// Number of slots available for producer advancement.
    pub fn available(self) -> u64 {
        u64::from(self.layout.depth()) - self.distance
    }
}
