//! Typed numeric values carried by ABI-v1 event records.

use crate::abi::{AbiDecodeError, EVENT_ACCESSES, EVENT_KINDS, EVENT_STAGES};

/// ABI-v1 event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiEventKind {
    /// The attempted start IOVA has no containing mapping.
    UnmappedIova,
    /// Checked address or extent arithmetic overflowed.
    RangeOverflow,
    /// A representable requested end exceeds one containing mapping.
    OutOfBounds,
    /// Descriptor version is not ABI v1.
    InvalidVersion,
    /// Descriptor size is not 64 bytes.
    InvalidSize,
    /// Descriptor opcode is not assigned by ABI v1.
    InvalidOpcode,
    /// Descriptor flags contain a nonzero bit.
    InvalidFlags,
    /// A required-zero descriptor payload word is nonzero.
    InvalidReserved,
    /// The broadcast destination count is zero or above its limit.
    InvalidIndirectCount,
    /// The broadcast destination-list base is not eight-byte aligned.
    InvalidIndirectAlignment,
    /// A scalar field exceeds its advertised fixed-width limit.
    ValueOutOfRange,
}

impl AbiEventKind {
    /// Returns the assigned ABI-v1 numeric value.
    pub const fn value(self) -> u16 {
        match self {
            Self::UnmappedIova => EVENT_KINDS[0].value,
            Self::RangeOverflow => EVENT_KINDS[1].value,
            Self::OutOfBounds => EVENT_KINDS[2].value,
            Self::InvalidVersion => EVENT_KINDS[3].value,
            Self::InvalidSize => EVENT_KINDS[4].value,
            Self::InvalidOpcode => EVENT_KINDS[5].value,
            Self::InvalidFlags => EVENT_KINDS[6].value,
            Self::InvalidReserved => EVENT_KINDS[7].value,
            Self::InvalidIndirectCount => EVENT_KINDS[8].value,
            Self::InvalidIndirectAlignment => EVENT_KINDS[9].value,
            Self::ValueOutOfRange => EVENT_KINDS[10].value,
        }
    }

    pub(super) const fn parse(value: u16) -> Result<Self, AbiDecodeError> {
        match value {
            value if value == EVENT_KINDS[0].value => Ok(Self::UnmappedIova),
            value if value == EVENT_KINDS[1].value => Ok(Self::RangeOverflow),
            value if value == EVENT_KINDS[2].value => Ok(Self::OutOfBounds),
            value if value == EVENT_KINDS[3].value => Ok(Self::InvalidVersion),
            value if value == EVENT_KINDS[4].value => Ok(Self::InvalidSize),
            value if value == EVENT_KINDS[5].value => Ok(Self::InvalidOpcode),
            value if value == EVENT_KINDS[6].value => Ok(Self::InvalidFlags),
            value if value == EVENT_KINDS[7].value => Ok(Self::InvalidReserved),
            value if value == EVENT_KINDS[8].value => Ok(Self::InvalidIndirectCount),
            value if value == EVENT_KINDS[9].value => Ok(Self::InvalidIndirectAlignment),
            value if value == EVENT_KINDS[10].value => Ok(Self::ValueOutOfRange),
            value => Err(AbiDecodeError::UnknownEventKind { value }),
        }
    }
}

/// Operation phase associated with an ABI-v1 event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiEventStage {
    /// Descriptor header, reserved, or scalar validation.
    Descriptor,
    /// Broadcast destination-list container access.
    IndirectList,
    /// Source operand access.
    Source,
    /// Destination operand access.
    Destination,
}

impl AbiEventStage {
    /// Returns the assigned ABI-v1 numeric value.
    pub const fn value(self) -> u16 {
        match self {
            Self::Descriptor => EVENT_STAGES[0].value,
            Self::IndirectList => EVENT_STAGES[1].value,
            Self::Source => EVENT_STAGES[2].value,
            Self::Destination => EVENT_STAGES[3].value,
        }
    }

    pub(super) const fn parse(value: u16) -> Result<Self, AbiDecodeError> {
        match value {
            value if value == EVENT_STAGES[0].value => Ok(Self::Descriptor),
            value if value == EVENT_STAGES[1].value => Ok(Self::IndirectList),
            value if value == EVENT_STAGES[2].value => Ok(Self::Source),
            value if value == EVENT_STAGES[3].value => Ok(Self::Destination),
            value => Err(AbiDecodeError::UnknownEventStage { value }),
        }
    }
}

/// Memory access associated with an ABI-v1 event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiEventAccess {
    /// Validation without a memory read or write.
    None,
    /// DMA read.
    Read,
    /// DMA write.
    Write,
}

/// Fixed-width auxiliary values accompanying an event kind and locator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiFaultDetails {
    detail0: u64,
    detail1: u64,
    stage: AbiEventStage,
    access: AbiEventAccess,
    index: u64,
}

impl AbiFaultDetails {
    /// Creates auxiliary fields for boundary decoding and negative validation tests.
    pub const fn new(detail0: u64, detail1: u64, site: AbiFaultSite) -> Self {
        Self {
            detail0,
            detail1,
            stage: site.stage,
            access: site.access,
            index: site.index,
        }
    }

    pub(super) const fn values(self) -> (u64, u64, AbiEventStage, AbiEventAccess, u64) {
        (
            self.detail0,
            self.detail1,
            self.stage,
            self.access,
            self.index,
        )
    }
}

/// Stage, access direction, and row/list/field index of a fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiFaultSite {
    stage: AbiEventStage,
    access: AbiEventAccess,
    index: u64,
}

impl AbiFaultSite {
    /// Creates one canonical fault-site candidate.
    pub const fn new(stage: AbiEventStage, access: AbiEventAccess, index: u64) -> Self {
        Self {
            stage,
            access,
            index,
        }
    }
}

/// Row number and stride used for checked strided start arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiStrideOffset {
    row: u64,
    stride: u64,
}

impl AbiStrideOffset {
    /// Creates one strided row-offset candidate.
    pub const fn new(row: u64, stride: u64) -> Self {
        Self { row, stride }
    }

    pub(super) const fn values(self) -> (u64, u64) {
        (self.row, self.stride)
    }
}

/// Mapped-region length and requested exclusive end for a bounds fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiBounds {
    region_length: u64,
    requested_end: u64,
}

impl AbiBounds {
    /// Creates one representable bounds-fault candidate.
    pub const fn new(region_length: u64, requested_end: u64) -> Self {
        Self {
            region_length,
            requested_end,
        }
    }

    pub(super) const fn values(self) -> (u64, u64) {
        (self.region_length, self.requested_end)
    }
}

impl AbiEventAccess {
    /// Returns the assigned ABI-v1 numeric value.
    pub const fn value(self) -> u16 {
        match self {
            Self::None => EVENT_ACCESSES[0].value,
            Self::Read => EVENT_ACCESSES[1].value,
            Self::Write => EVENT_ACCESSES[2].value,
        }
    }

    pub(super) const fn parse(value: u16) -> Result<Self, AbiDecodeError> {
        match value {
            value if value == EVENT_ACCESSES[0].value => Ok(Self::None),
            value if value == EVENT_ACCESSES[1].value => Ok(Self::Read),
            value if value == EVENT_ACCESSES[2].value => Ok(Self::Write),
            value => Err(AbiDecodeError::UnknownEventAccess { value }),
        }
    }
}
