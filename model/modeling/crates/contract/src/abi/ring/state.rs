use super::{AbiRingEntry, RingLayout};
use crate::abi::{AbiAddressError, AbiControlError, DmaAddress};

/// One complete monotonic cursor published through the ABI control surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RingCursor(u64);

impl RingCursor {
    /// Creates a cursor from its complete fixed-width value.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the complete fixed-width value.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Advances the cursor without wrapping.
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

/// ABI-visible layout and producer/consumer state for one typed ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingState<R: AbiRingEntry> {
    layout: RingLayout<R>,
    producer: RingCursor,
    consumer: RingCursor,
    occupied: u64,
}

impl<R: AbiRingEntry> RingState<R> {
    /// Creates an empty state for a validated layout.
    pub const fn empty(layout: RingLayout<R>) -> Self {
        Self {
            layout,
            producer: RingCursor::new(0),
            consumer: RingCursor::new(0),
            occupied: 0,
        }
    }

    /// Validates ABI cursor ordering and retained capacity.
    pub fn try_new(
        layout: RingLayout<R>,
        producer: RingCursor,
        consumer: RingCursor,
    ) -> Result<Self, AbiControlError> {
        let Some(occupied) = producer.raw().checked_sub(consumer.raw()) else {
            return Err(AbiControlError::CursorRegression {
                producer: producer.raw(),
                consumer: consumer.raw(),
            });
        };
        if occupied > u64::from(layout.depth()) {
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
            occupied,
        })
    }

    /// Returns the typed DMA entry layout.
    pub const fn layout(self) -> RingLayout<R> {
        self.layout
    }

    /// Returns the published producer cursor.
    pub const fn producer(self) -> RingCursor {
        self.producer
    }

    /// Returns the published consumer cursor.
    pub const fn consumer(self) -> RingCursor {
        self.consumer
    }

    /// Returns whether every retained slot is occupied.
    pub fn is_full(self) -> bool {
        self.occupied == u64::from(self.layout.depth())
    }

    /// Applies a forward or idempotent producer publication.
    pub fn update_producer(self, proposed: RingCursor) -> Result<Self, AbiControlError> {
        if proposed < self.producer {
            return Err(AbiControlError::CursorUpdateRegression {
                current: self.producer.raw(),
                proposed: proposed.raw(),
            });
        }
        Self::try_new(self.layout, proposed, self.consumer)
    }

    /// Applies a forward or idempotent consumer publication.
    pub fn update_consumer(self, proposed: RingCursor) -> Result<Self, AbiControlError> {
        if proposed < self.consumer {
            return Err(AbiControlError::CursorUpdateRegression {
                current: self.consumer.raw(),
                proposed: proposed.raw(),
            });
        }
        Self::try_new(self.layout, self.producer, proposed)
    }

    /// Returns the DMA address selected by one ABI cursor.
    pub fn entry_address(self, cursor: RingCursor) -> Result<DmaAddress, AbiAddressError> {
        self.layout.entry_address(cursor)
    }
}
