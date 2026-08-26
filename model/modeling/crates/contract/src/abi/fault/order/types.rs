//! Typed positions for canonical ABI-v1 fault selection.

use crate::abi::descriptor::DescriptorViolationKind;

/// One operand visit in exact per-operation execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiOperandVisit {
    /// Copy source.
    CopySource,
    /// Copy destination.
    CopyDestination,
    /// Strided source for one row.
    StridedSource {
        /// Zero-based row number.
        row: u64,
    },
    /// Strided destination for one row.
    StridedDestination {
        /// Zero-based row number.
        row: u64,
    },
    /// Broadcast source.
    BroadcastSource,
    /// Broadcast destination-list container.
    BroadcastList,
    /// One broadcast destination.
    BroadcastDestination {
        /// Zero-based destination index.
        index: u64,
    },
    /// Vector-add source A.
    VectorSourceA,
    /// Vector-add source B.
    VectorSourceB,
    /// Vector-add destination.
    VectorDestination,
}

/// Checks within one visited operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiOperandFaultStep {
    /// Checked row-start arithmetic.
    CheckedStart,
    /// Checked exclusive-end arithmetic.
    CheckedEnd,
    /// Containing-mapping lookup.
    Unmapped,
    /// End against one containing mapping.
    OutOfBounds,
}

/// Typed location of one candidate in ABI-v1 first-match order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiFaultOrder {
    /// One generated descriptor-validation location.
    Descriptor(DescriptorViolationKind),
    /// One check within one visited operand.
    Operand {
        /// Exact operation-specific visit.
        visit: AbiOperandVisit,
        /// Exact check within the visit.
        step: AbiOperandFaultStep,
    },
}

/// Failure while assembling or selecting canonical fault candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiFaultSelectionError {
    /// The fault's auxiliary tuple is not canonical.
    InvalidFaultTuple,
    /// The fault kind or tuple does not match its typed order location.
    FaultDoesNotMatchOrder,
    /// Candidates from multiple operations were mixed.
    MixedOperations,
    /// Two candidates claim the same earliest typed order location.
    AmbiguousFirstFault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct OrderKey {
    class: u8,
    major: u64,
    index: u64,
    side: u8,
    step: u8,
}

impl OrderKey {
    pub(super) const fn new(class: u8, major: u64, index: u64, side: u8, step: u8) -> Self {
        Self {
            class,
            major,
            index,
            side,
            step,
        }
    }
}
