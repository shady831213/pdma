use crate::semantic::{SemanticError, SemanticResult};

/// One monotonic queue cursor shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RingCursor(u64);

impl RingCursor {
    /// Creates a cursor from its complete monotonic value.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Complete monotonic cursor value.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Advances without wrapping.
    pub const fn checked_increment(self, increment: u64) -> SemanticResult<Self> {
        match self.0.checked_add(increment) {
            Some(next) => Ok(Self(next)),
            None => Err(SemanticError::CursorIncrementOverflow {
                cursor: self.0,
                increment,
            }),
        }
    }
}

/// Producer and consumer shadows bounded by one semantic ring capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingCursors {
    depth: u32,
    producer: RingCursor,
    consumer: RingCursor,
    distance: u64,
}

impl RingCursors {
    /// Validates cursor ordering and retained-capacity distance.
    pub fn try_new(depth: u32, producer: RingCursor, consumer: RingCursor) -> SemanticResult<Self> {
        if depth == 0 {
            return Err(SemanticError::ZeroCapacity);
        }
        let Some(distance) = producer.raw().checked_sub(consumer.raw()) else {
            return Err(SemanticError::CursorRegression {
                producer: producer.raw(),
                consumer: consumer.raw(),
            });
        };
        if distance > u64::from(depth) {
            return Err(SemanticError::CursorDistanceExceeded {
                producer: producer.raw(),
                consumer: consumer.raw(),
                depth,
            });
        }
        Ok(Self {
            depth,
            producer,
            consumer,
            distance,
        })
    }

    /// Applies a forward or idempotent producer publication.
    pub fn update_producer(self, proposed: RingCursor) -> SemanticResult<Self> {
        if proposed.raw() < self.producer.raw() {
            return Err(SemanticError::CursorUpdateRegression {
                current: self.producer.raw(),
                proposed: proposed.raw(),
            });
        }
        Self::try_new(self.depth, proposed, self.consumer)
    }

    /// Applies a forward or idempotent consumer publication.
    pub fn update_consumer(self, proposed: RingCursor) -> SemanticResult<Self> {
        if proposed.raw() < self.consumer.raw() {
            return Err(SemanticError::CursorUpdateRegression {
                current: self.consumer.raw(),
                proposed: proposed.raw(),
            });
        }
        Self::try_new(self.depth, self.producer, proposed)
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
        self.distance == u64::from(self.depth)
    }

    /// Number of slots available for producer advancement.
    pub fn available(self) -> u64 {
        u64::from(self.depth) - self.distance
    }
}
