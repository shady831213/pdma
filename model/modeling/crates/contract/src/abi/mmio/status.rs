/// Typed value encoded by `I_RSP_STATE` bits 0 through 6.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrqSourceStatus(u32);

impl IrqSourceStatus {
    /// Mask containing every assigned state bit.
    pub const MASK: u32 = 0x7f;
    /// Encodes the complete source state.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Reports whether the source exists.
    pub const fn present(self) -> bool {
        self.0 & 1 != 0
    }
    /// Reports whether delivery is enabled.
    pub const fn enabled(self) -> bool {
        self.0 & 2 != 0
    }
    /// Reports whether delivery is masked.
    pub const fn masked(self) -> bool {
        self.0 & 4 != 0
    }
    /// Reports whether a source cause is pending.
    pub const fn cause_pending(self) -> bool {
        self.0 & 8 != 0
    }
    /// Reports whether a notification is deliverable.
    pub const fn deliverable(self) -> bool {
        self.0 & 0x10 != 0
    }
    /// Reports whether the line is asserted.
    pub const fn line_asserted(self) -> bool {
        self.0 & 0x20 != 0
    }
    /// Reports whether source records remain pending.
    pub const fn records_pending(self) -> bool {
        self.0 & 0x40 != 0
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
