use pdma_model_contract::ChannelFaultCode;
use std::fmt::{self, Display, Formatter};

/// Sticky channel faults cleared only by channel or device reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The command ring could not be read.
    CommandRead,
    /// A completion record could not be written.
    ResponseWrite,
}

impl Fault {
    /// Value latched in the channel's fault register.
    pub const fn code(self) -> ChannelFaultCode {
        match self {
            Self::CommandRead => ChannelFaultCode::CommandRead,
            Self::ResponseWrite => ChannelFaultCode::ResponseWrite,
        }
    }
}

/// Bus rejection or terminal channel failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// No register starts at the supplied offset.
    Unmapped,
    /// Transaction width does not match the register.
    Width,
    /// Register permissions forbid this access.
    Access,
    /// A write changes reserved bits.
    ReservedBits,
    /// Ring geometry is immutable after first enable.
    ConfigurationLocked,
    /// Ring capacity, alignment, or extent is invalid.
    Configuration,
    /// Producer or consumer advancement exceeds available entries.
    Cursor,
    /// A hardware fault was latched during this operation.
    Fault(Fault),
    /// The channel remains stopped after an earlier hardware fault.
    Halted,
    /// Hardware reset invalidated this retained operation before another interaction.
    Reset,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA channel: {self:?}")
    }
}
impl std::error::Error for Error {}
