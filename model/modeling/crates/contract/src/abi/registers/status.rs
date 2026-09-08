/// One assigned bit in the IRQ source-status register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrqStateBit {
    /// Source existence bit.
    Present,
    /// Delivery-enabled bit.
    Enabled,
    /// Delivery-masked bit.
    Masked,
    /// Pending-cause bit.
    CausePending,
    /// Deliverable-notification bit.
    Deliverable,
    /// Asserted-line bit.
    LineAsserted,
    /// Pending-records bit.
    RecordsPending,
}

impl IrqStateBit {
    /// Every assigned state bit in ascending bit order.
    pub const ALL: [Self; 7] = [
        Self::Present,
        Self::Enabled,
        Self::Masked,
        Self::CausePending,
        Self::Deliverable,
        Self::LineAsserted,
        Self::RecordsPending,
    ];

    /// Returns the canonical uppercase state-bit name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Present => "PRESENT",
            Self::Enabled => "ENABLED",
            Self::Masked => "MASKED",
            Self::CausePending => "CAUSE_PENDING",
            Self::Deliverable => "DELIVERABLE",
            Self::LineAsserted => "LINE_ASSERTED",
            Self::RecordsPending => "RECORDS_PENDING",
        }
    }

    /// Returns the assigned single-bit mask.
    pub const fn mask(self) -> u32 {
        match self {
            Self::Present => 1,
            Self::Enabled => 2,
            Self::Masked => 4,
            Self::CausePending => 8,
            Self::Deliverable => 0x10,
            Self::LineAsserted => 0x20,
            Self::RecordsPending => 0x40,
        }
    }
}

/// Typed value encoded by the IRQ status register's bits zero through six.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrqSourceStatus(u32);

impl IrqSourceStatus {
    /// Mask containing every assigned state bit.
    pub const MASK: u32 = IrqStateBit::Present.mask()
        | IrqStateBit::Enabled.mask()
        | IrqStateBit::Masked.mask()
        | IrqStateBit::CausePending.mask()
        | IrqStateBit::Deliverable.mask()
        | IrqStateBit::LineAsserted.mask()
        | IrqStateBit::RecordsPending.mask();
    /// Encodes the complete source state.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Reports whether the source exists.
    pub const fn present(self) -> bool {
        self.0 & IrqStateBit::Present.mask() != 0
    }
    /// Reports whether delivery is enabled.
    pub const fn enabled(self) -> bool {
        self.0 & IrqStateBit::Enabled.mask() != 0
    }
    /// Reports whether delivery is masked.
    pub const fn masked(self) -> bool {
        self.0 & IrqStateBit::Masked.mask() != 0
    }
    /// Reports whether a source cause is pending.
    pub const fn cause_pending(self) -> bool {
        self.0 & IrqStateBit::CausePending.mask() != 0
    }
    /// Reports whether a notification is deliverable.
    pub const fn deliverable(self) -> bool {
        self.0 & IrqStateBit::Deliverable.mask() != 0
    }
    /// Reports whether the line is asserted.
    pub const fn line_asserted(self) -> bool {
        self.0 & IrqStateBit::LineAsserted.mask() != 0
    }
    /// Reports whether source records remain pending.
    pub const fn records_pending(self) -> bool {
        self.0 & IrqStateBit::RecordsPending.mask() != 0
    }
}

/// Reserved bits found in an encoded IRQ source state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrqSourceStatusError {
    /// Reserved bits which were set.
    pub bits: u32,
}

impl TryFrom<u32> for IrqSourceStatus {
    type Error = IrqSourceStatusError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let reserved = value & !Self::MASK;
        if reserved != 0 {
            return Err(IrqSourceStatusError { bits: reserved });
        }
        Ok(Self(value))
    }
}
