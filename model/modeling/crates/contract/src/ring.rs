use core::fmt;
use hardware_interface_schema::{SchemaCodec, checked_align_up};

/// Shared ring calculations; implementations own their state and I/O operations.
///
/// Pointer snapshots are supplied by the caller, so acquiring or publishing them
/// may be synchronous or asynchronous without imposing either on this trait.
pub trait Ring {
    /// Wire record stored in each slot.
    type Entry: SchemaCodec;

    /// Current ring base address.
    fn base(&self) -> u64;

    /// Current capacity in entries.
    fn capacity(&self) -> u32;

    /// Wire-layout distance between entries, independent of ring configuration.
    fn entry_stride(&self) -> Result<u64, RingError> {
        checked_align_up(Self::Entry::EXTENT, Self::Entry::ALIGNMENT)
            .and_then(|size| u64::try_from(size).ok())
            .filter(|stride| *stride != 0)
            .ok_or(RingError::Geometry)
    }

    /// Checks alignment, capacity and the complete wire-layout extent.
    fn validate_geometry(&self) -> Result<(), RingError> {
        let stride = self.entry_stride()?;
        let alignment = u64::try_from(Self::Entry::ALIGNMENT).map_err(|_| RingError::Geometry)?;
        let base = self.base();
        let capacity = self.capacity();
        if !capacity.is_power_of_two()
            || capacity > u32::MAX / 2
            || base.checked_rem(alignment) != Some(0)
        {
            return Err(RingError::Geometry);
        }
        u64::from(capacity)
            .checked_mul(stride)
            .and_then(|size| base.checked_add(size))
            .ok_or(RingError::Geometry)?;
        Ok(())
    }

    /// Computes an entry address without reading memory or changing pointers.
    fn entry(&self, pointer: u32) -> Result<u64, RingError> {
        self.validate_geometry()?;
        Ok(self.base() + u64::from(pointer % self.capacity()) * self.entry_stride()?)
    }

    /// Checks snapshot occupancy against capacity, without publishing or validating history.
    fn validate_pointers(&self, pointers: RingPointers) -> Result<(), RingError> {
        if pointers.pending() > self.capacity() {
            return Err(RingError::Pointers);
        }
        Ok(())
    }

    /// Checks a producer publication against this ring's current capacity.
    fn with_producer(&self, pointers: RingPointers, next: u32) -> Result<RingPointers, RingError> {
        self.validate_pointers(pointers)?;
        let pending = pointers.pending();
        let capacity = self.capacity();
        if next.wrapping_sub(pointers.producer) > capacity - pending {
            return Err(RingError::Pointers);
        }
        Ok(RingPointers {
            producer: next,
            ..pointers
        })
    }

    /// Checks that a consumer publication removes only published entries.
    fn with_consumer(&self, pointers: RingPointers, next: u32) -> Result<RingPointers, RingError> {
        self.validate_pointers(pointers)?;
        let pending = pointers.pending();
        if next.wrapping_sub(pointers.consumer) > pending {
            return Err(RingError::Pointers);
        }
        Ok(RingPointers {
            consumer: next,
            ..pointers
        })
    }

    /// Calculates a producer advance without publishing it.
    fn advance_producer(
        &self,
        pointers: RingPointers,
        count: u32,
    ) -> Result<RingPointers, RingError> {
        self.with_producer(pointers, pointers.producer.wrapping_add(count))
    }

    /// Calculates a consumer advance without publishing it.
    fn advance_consumer(
        &self,
        pointers: RingPointers,
        count: u32,
    ) -> Result<RingPointers, RingError> {
        self.with_consumer(pointers, pointers.consumer.wrapping_add(count))
    }
}

/// A snapshot of the authoritative producer/consumer cursor storage.
///
/// Returned updates are pure calculations. Each endpoint publishes the updated
/// pointer only after its memory access succeeds; this type never publishes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RingPointers {
    /// Monotonic wrapping count of published entries.
    pub producer: u32,
    /// Monotonic wrapping count of consumed entries.
    pub consumer: u32,
}

impl RingPointers {
    /// Number of published but unconsumed entries in this snapshot.
    pub const fn pending(self) -> u32 {
        self.producer.wrapping_sub(self.consumer)
    }

    /// Whether the producer and consumer point to the same logical position.
    pub const fn is_empty(self) -> bool {
        self.producer == self.consumer
    }
}

/// Invalid shared ring geometry or pointer movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingError {
    /// Capacity, alignment, entry stride, or complete address extent is invalid.
    Geometry,
    /// Pointers retreat, exceed capacity, overwrite unread data, or consume absent data.
    Pointers,
}

impl fmt::Display for RingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Geometry => f.write_str("invalid ring geometry"),
            Self::Pointers => f.write_str("invalid ring pointer movement"),
        }
    }
}

impl std::error::Error for RingError {}
