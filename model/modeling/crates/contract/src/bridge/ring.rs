use super::PdmaMemoryRegion;
use crate::abi::{AbiControlError, MAX_RING_DEPTH, RingCursor, RingRecord};

/// One validated ABI-v1 ring region and its retained full-capacity depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingLayout {
    region: PdmaMemoryRegion,
    depth: u32,
    extent: u64,
    record: RingRecord,
}

impl RingLayout {
    /// Validates depth, extent, alignment, and exact region length.
    pub fn try_new(
        region: PdmaMemoryRegion,
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
        if region.base().raw() % record.alignment() != 0 {
            return Err(AbiControlError::MisalignedRing {
                base: region.base().raw(),
                alignment: record.alignment(),
            });
        }
        if region.length() != extent {
            return Err(AbiControlError::RingExtentMismatch {
                expected: extent,
                actual: region.length(),
            });
        }
        Ok(Self {
            region,
            depth,
            extent,
            record,
        })
    }

    /// Multiplies a fixed-width depth by a canonical record width without wrapping.
    pub fn checked_extent(depth: u64, record_size: u16) -> Result<u64, AbiControlError> {
        depth
            .checked_mul(u64::from(record_size))
            .ok_or(AbiControlError::RingExtentOverflow { depth, record_size })
    }

    /// Backing DMA region.
    pub const fn region(self) -> PdmaMemoryRegion {
        self.region
    }

    /// Retained ring capacity.
    pub const fn depth(self) -> u32 {
        self.depth
    }

    /// Exact byte extent.
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

/// A validated producer/consumer pair for one ring.
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
