//! Typed values carried by canonical ABI-v1 faults.

pub use crate::abi::record::{AbiEventAccess, AbiEventKind, AbiEventStage};

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
