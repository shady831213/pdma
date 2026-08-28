//! Fixed-width ABI-v1 ring-entry geometry.

use core::marker::PhantomData;

use super::{
    AbiAddressError, AbiCommand, AbiCompletion, AbiControlError, AbiEvent, COMPLETION_ALIGNMENT,
    COMPLETION_SIZE, DESCRIPTOR_ALIGNMENT, DESCRIPTOR_SIZE, DmaAddress, DmaRegion, EVENT_ALIGNMENT,
    EVENT_SIZE, MAX_RING_DEPTH,
};

mod private {
    pub trait Sealed {}
}

/// ABI record type permitted as a DMA ring entry.
pub trait AbiRingEntry: private::Sealed + Copy + core::fmt::Debug + Eq {
    /// Fixed encoded entry width in bytes.
    const ENTRY_SIZE: u16;
    /// Required DMA ring-base alignment in bytes.
    const RING_ALIGNMENT: u64;
}

impl private::Sealed for AbiCommand {}
impl AbiRingEntry for AbiCommand {
    const ENTRY_SIZE: u16 = DESCRIPTOR_SIZE;
    const RING_ALIGNMENT: u64 = DESCRIPTOR_ALIGNMENT;
}

impl private::Sealed for AbiCompletion {}
impl AbiRingEntry for AbiCompletion {
    const ENTRY_SIZE: u16 = COMPLETION_SIZE;
    const RING_ALIGNMENT: u64 = COMPLETION_ALIGNMENT;
}

impl private::Sealed for AbiEvent {}
impl AbiRingEntry for AbiEvent {
    const ENTRY_SIZE: u16 = EVENT_SIZE;
    const RING_ALIGNMENT: u64 = EVENT_ALIGNMENT;
}

/// Exact DMA entry geometry for one typed ABI ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingLayout<R: AbiRingEntry> {
    region: DmaRegion,
    depth: u32,
    entry: PhantomData<fn() -> R>,
}

impl<R: AbiRingEntry> RingLayout<R> {
    /// Validates depth, alignment, and exact DMA extent.
    pub fn try_new(region: DmaRegion, depth: u64) -> Result<Self, AbiControlError> {
        let depth = match u32::try_from(depth) {
            Ok(value) if value != 0 => value,
            Ok(_) | Err(_) => {
                return Err(AbiControlError::InvalidRingDepth {
                    depth,
                    maximum: MAX_RING_DEPTH,
                });
            }
        };
        let extent = Self::checked_extent(u64::from(depth))?;
        if region.base().raw() % R::RING_ALIGNMENT != 0 {
            return Err(AbiControlError::MisalignedRing {
                base: region.base().raw(),
                alignment: R::RING_ALIGNMENT,
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
            entry: PhantomData,
        })
    }

    /// Computes the exact byte extent for a depth without wrapping.
    pub fn checked_extent(depth: u64) -> Result<u64, AbiControlError> {
        depth.checked_mul(u64::from(R::ENTRY_SIZE)).ok_or({
            AbiControlError::RingExtentOverflow {
                depth,
                record_size: R::ENTRY_SIZE,
            }
        })
    }

    /// Backing DMA entry extent.
    pub const fn region(self) -> DmaRegion {
        self.region
    }

    /// Configured retained entry count.
    pub const fn depth(self) -> u32 {
        self.depth
    }

    /// Derived exact byte extent.
    pub const fn extent(self) -> u64 {
        self.region.length()
    }

    /// Fixed encoded entry width in bytes.
    pub const fn entry_size(self) -> u16 {
        R::ENTRY_SIZE
    }

    /// Required DMA ring-base alignment in bytes.
    pub const fn alignment(self) -> u64 {
        R::RING_ALIGNMENT
    }

    /// Physical entry index selected by a monotonic cursor.
    pub fn slot(self, cursor: u64) -> u64 {
        cursor % u64::from(self.depth)
    }

    /// DMA address of the entry selected by a monotonic cursor.
    pub fn entry_address(self, cursor: u64) -> Result<DmaAddress, AbiAddressError> {
        let offset = self
            .slot(cursor)
            .checked_mul(u64::from(R::ENTRY_SIZE))
            .ok_or_else(|| AbiAddressError::RangeOverflow {
                base: self.region.base().raw(),
                length: u64::from(R::ENTRY_SIZE),
            })?;
        self.region.base().checked_add(offset)
    }
}

/// Typed entry layouts assigned to the three queue ring roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueRingLayouts {
    submission: RingLayout<AbiCommand>,
    completion: RingLayout<AbiCompletion>,
    event: RingLayout<AbiEvent>,
}

impl QueueRingLayouts {
    /// Groups submission, completion, and event entry layouts.
    pub const fn new(
        submission: RingLayout<AbiCommand>,
        completion: RingLayout<AbiCompletion>,
        event: RingLayout<AbiEvent>,
    ) -> Self {
        Self {
            submission,
            completion,
            event,
        }
    }

    /// Submission-ring entry layout.
    pub const fn submission(self) -> RingLayout<AbiCommand> {
        self.submission
    }

    /// Completion-ring entry layout.
    pub const fn completion(self) -> RingLayout<AbiCompletion> {
        self.completion
    }

    /// Event-ring entry layout.
    pub const fn event(self) -> RingLayout<AbiEvent> {
        self.event
    }
}
