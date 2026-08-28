//! Canonical ABI-v1 fault locator and auxiliary tuple.

mod descriptor;
mod order;
mod validation;
mod value;

pub use order::{
    AbiFaultCandidate, AbiFaultOrder, AbiFaultSelectionError, AbiOperandFaultStep, AbiOperandVisit,
    select_canonical_fault,
};
pub use value::{
    AbiBounds, AbiEventAccess, AbiEventKind, AbiEventStage, AbiFaultDetails, AbiFaultSite,
    AbiStrideOffset,
};

use crate::abi::descriptor::{AbiCommandHeader, location_offset};
use crate::abi::{
    ABI_VERSION, DESCRIPTOR_SIZE, DmaAddress, INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS,
    MAX_TRANSFER_BYTES, VECTOR_ADD_OPCODE,
};

/// Canonical address-aware fault carried by one ABI event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiFault {
    kind: AbiEventKind,
    locator: DmaAddress,
    details: AbiFaultDetails,
}

#[derive(Clone, Copy)]
pub(super) struct FaultTuple {
    pub(super) kind: AbiEventKind,
    pub(super) locator: u64,
    pub(super) observed: u64,
    pub(super) expected: u64,
    pub(super) stage: u16,
    pub(super) access: u16,
    pub(super) index: u64,
}

impl AbiFault {
    /// Creates a tuple for boundary decoding and negative validation tests.
    pub const fn from_parts(
        kind: AbiEventKind,
        locator: DmaAddress,
        details: AbiFaultDetails,
    ) -> Self {
        Self {
            kind,
            locator,
            details,
        }
    }

    /// Reports an unsupported descriptor version.
    pub fn invalid_version(locator: DmaAddress, observed: u16) -> Self {
        Self::from_parts(
            AbiEventKind::InvalidVersion,
            locator,
            AbiFaultDetails::new(
                u64::from(observed),
                u64::from(ABI_VERSION),
                descriptor_site(location_offset(
                    AbiCommandHeader::VERSION_IDENTITY.at_byte(AbiCommandHeader::VERSION_OFFSET),
                )),
            ),
        )
    }

    /// Reports an unsupported descriptor record size.
    pub fn invalid_size(locator: DmaAddress, observed: u16) -> Self {
        Self::from_parts(
            AbiEventKind::InvalidSize,
            locator,
            AbiFaultDetails::new(
                u64::from(observed),
                u64::from(DESCRIPTOR_SIZE),
                descriptor_site(location_offset(
                    AbiCommandHeader::SIZE_IDENTITY.at_byte(AbiCommandHeader::SIZE_OFFSET),
                )),
            ),
        )
    }

    /// Reports an unsupported descriptor opcode.
    pub fn invalid_opcode(locator: DmaAddress, observed: u16) -> Self {
        Self::from_parts(
            AbiEventKind::InvalidOpcode,
            locator,
            AbiFaultDetails::new(
                u64::from(observed),
                u64::from(VECTOR_ADD_OPCODE),
                descriptor_site(location_offset(
                    AbiCommandHeader::OPCODE_IDENTITY.at_byte(AbiCommandHeader::OPCODE_OFFSET),
                )),
            ),
        )
    }

    /// Reports nonzero descriptor flags.
    pub fn invalid_flags(locator: DmaAddress, observed: u16) -> Self {
        Self::from_parts(
            AbiEventKind::InvalidFlags,
            locator,
            AbiFaultDetails::new(
                u64::from(observed),
                0,
                descriptor_site(location_offset(
                    AbiCommandHeader::FLAGS_IDENTITY.at_byte(AbiCommandHeader::FLAGS_OFFSET),
                )),
            ),
        )
    }

    /// Reports the first nonzero reserved payload word.
    pub const fn invalid_reserved(locator: DmaAddress, offset: u64, observed: u64) -> Self {
        Self::from_parts(
            AbiEventKind::InvalidReserved,
            locator,
            AbiFaultDetails::new(observed, 0, descriptor_site(offset)),
        )
    }

    /// Reports the first scalar exceeding its fixed-width advertised limit.
    pub fn value_out_of_range(locator: DmaAddress, offset: u64, observed: u64) -> Self {
        Self::from_parts(
            AbiEventKind::ValueOutOfRange,
            locator,
            AbiFaultDetails::new(
                observed,
                u64::from(MAX_TRANSFER_BYTES),
                descriptor_site(offset),
            ),
        )
    }

    /// Reports a zero or over-limit broadcast destination count.
    pub fn invalid_indirect_count(locator: DmaAddress, observed: u64) -> Self {
        Self::from_parts(
            AbiEventKind::InvalidIndirectCount,
            locator,
            AbiFaultDetails::new(
                observed,
                u64::from(MAX_INDIRECT_DESTINATIONS),
                AbiFaultSite::new(AbiEventStage::IndirectList, AbiEventAccess::None, 0),
            ),
        )
    }

    /// Reports a misaligned broadcast destination-list base.
    pub const fn invalid_indirect_alignment(locator: DmaAddress) -> Self {
        Self::from_parts(
            AbiEventKind::InvalidIndirectAlignment,
            locator,
            AbiFaultDetails::new(
                locator.raw() % INDIRECT_ENTRY_ALIGNMENT,
                INDIRECT_ENTRY_ALIGNMENT,
                AbiFaultSite::new(AbiEventStage::IndirectList, AbiEventAccess::None, 0),
            ),
        )
    }

    /// Reports direct checked exclusive-end overflow.
    pub const fn range_overflow_direct(
        locator: DmaAddress,
        length: u64,
        site: AbiFaultSite,
    ) -> Self {
        Self::from_parts(
            AbiEventKind::RangeOverflow,
            locator,
            AbiFaultDetails::new(length, 0, site),
        )
    }

    /// Reports checked strided row-start arithmetic overflow.
    pub const fn range_overflow_strided(
        locator: DmaAddress,
        offset: AbiStrideOffset,
        site: AbiFaultSite,
    ) -> Self {
        let (row, stride) = offset.values();
        Self::from_parts(
            AbiEventKind::RangeOverflow,
            locator,
            AbiFaultDetails::new(row, stride, site),
        )
    }

    /// Reports checked broadcast list-container extent overflow.
    pub const fn range_overflow_indirect(locator: DmaAddress, count: u64) -> Self {
        Self::from_parts(
            AbiEventKind::RangeOverflow,
            locator,
            AbiFaultDetails::new(
                count,
                INDIRECT_ENTRY_ALIGNMENT,
                AbiFaultSite::new(AbiEventStage::IndirectList, AbiEventAccess::Read, 0),
            ),
        )
    }

    /// Reports an absent mapping at an attempted operand start.
    pub const fn unmapped(locator: DmaAddress, length: u64, site: AbiFaultSite) -> Self {
        Self::from_parts(
            AbiEventKind::UnmappedDmaAddress,
            locator,
            AbiFaultDetails::new(length, 0, site),
        )
    }

    /// Reports a representable end beyond one containing mapping.
    pub const fn out_of_bounds(locator: DmaAddress, bounds: AbiBounds, site: AbiFaultSite) -> Self {
        let (region_length, requested_end) = bounds.values();
        Self::from_parts(
            AbiEventKind::OutOfBounds,
            locator,
            AbiFaultDetails::new(region_length, requested_end, site),
        )
    }

    /// Returns kind, locator, details, stage, access, and index as fixed-width values.
    pub const fn tuple(self) -> (AbiEventKind, u64, u64, u64, u16, u16, u64) {
        let tuple = self.parts();
        (
            tuple.kind,
            tuple.locator,
            tuple.observed,
            tuple.expected,
            tuple.stage,
            tuple.access,
            tuple.index,
        )
    }

    pub(super) const fn parts(self) -> FaultTuple {
        let (detail0, detail1, stage, access, index) = self.details.values();
        FaultTuple {
            kind: self.kind,
            locator: self.locator.raw(),
            observed: detail0,
            expected: detail1,
            stage: stage.value(),
            access: access.value(),
            index,
        }
    }

    /// Returns the assigned event kind.
    pub const fn kind(self) -> AbiEventKind {
        self.kind
    }

    pub(super) fn is_canonical(self) -> bool {
        validation::is_canonical(self)
    }
}

const fn descriptor_site(index: u64) -> AbiFaultSite {
    AbiFaultSite::new(AbiEventStage::Descriptor, AbiEventAccess::None, index)
}
