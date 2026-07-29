//! Typed first-match positions for ABI-v1 fault candidates.

/// Descriptor header fields in their canonical validation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiHeaderField {
    /// Descriptor version at byte zero.
    Version,
    /// Descriptor record size at byte six.
    Size,
    /// Descriptor opcode at byte two.
    Opcode,
    /// Descriptor flags at byte four.
    Flags,
}

/// Operation-specific required-zero words ordered by payload offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiReservedField {
    /// Copy word at byte 40.
    CopyOffset40,
    /// Copy word at byte 48.
    CopyOffset48,
    /// Copy word at byte 56.
    CopyOffset56,
    /// Broadcast word at byte 48.
    BroadcastOffset48,
    /// Broadcast word at byte 56.
    BroadcastOffset56,
    /// Vector-add word at byte 48.
    VectorOffset48,
    /// Vector-add word at byte 56.
    VectorOffset56,
}

/// Non-address fields in their operation-specific validation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiScalarField {
    /// Copy transfer length at byte 32.
    CopyLength,
    /// Strided row count at byte 32.
    StridedRows,
    /// Strided row length at byte 40.
    StridedRowBytes,
    /// Strided source stride at byte 48.
    StridedSourceStride,
    /// Strided destination stride at byte 56.
    StridedDestinationStride,
    /// Broadcast transfer length at byte 32.
    BroadcastLength,
    /// Broadcast destination count at byte 40.
    BroadcastDestinationCount,
    /// Broadcast list-base alignment after count validation.
    BroadcastListAlignment,
    /// Vector-add transfer length at byte 40.
    VectorLength,
}

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
    /// Descriptor header validation.
    Header(AbiHeaderField),
    /// Required-zero payload validation.
    Reserved(AbiReservedField),
    /// Non-address scalar validation.
    Scalar(AbiScalarField),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    Copy,
    Strided,
    Broadcast,
    Vector,
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
