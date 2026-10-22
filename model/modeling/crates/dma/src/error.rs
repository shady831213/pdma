use pdma_model_contract::dma::ChannelFatalCode;
use std::fmt::{self, Display, Formatter};

/// Sticky channel fatal failures cleared only by channel or device reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelFatal {
    /// The command ring could not be read.
    CommandRead,
    /// A stopping-error event could not be written.
    EventWrite,
    /// Error reporting found no free event slot.
    EventFull,
    /// Error reporting found invalid event-ring geometry.
    EventConfiguration,
}

impl ChannelFatal {
    /// Value latched in the channel's fatal register.
    pub const fn code(self) -> ChannelFatalCode {
        match self {
            Self::CommandRead => ChannelFatalCode::CommandRead,
            Self::EventWrite => ChannelFatalCode::EventWrite,
            Self::EventFull => ChannelFatalCode::EventFull,
            Self::EventConfiguration => ChannelFatalCode::EventConfiguration,
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
    /// Reset cleanup is pending; configuration writes are temporarily rejected.
    ResetInProgress,
    /// A captured context frontier prevents conflicting channel or device changes.
    MaintenanceInProgress,
    /// Ring capacity, alignment, or extent is invalid.
    Configuration,
    /// Producer or consumer advancement exceeds available entries.
    Cursor,
    /// A hardware fatal was latched during this operation.
    ChannelFatal(ChannelFatal),
    /// The channel remains stopped after a stopping event error or hardware fatal.
    Halted,
    /// Hardware reset invalidated this retained operation before another interaction.
    Reset,
    /// A composition selected an invalid destination or returned an invalid result shape.
    Routing,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "PDMA channel: {self:?}")
    }
}
impl std::error::Error for Error {}

impl From<hardware_interface_schema::RegisterAccessError<Self>> for Error {
    fn from(error: hardware_interface_schema::RegisterAccessError<Self>) -> Self {
        use hardware_interface_schema::RegisterAccessError;
        match error {
            RegisterAccessError::OutsideWindow { .. }
            | RegisterAccessError::InvalidOffset { .. } => Self::Unmapped,
            RegisterAccessError::WidthMismatch { .. }
            | RegisterAccessError::InvalidLayout { .. } => Self::Width,
            RegisterAccessError::ReadForbidden { .. }
            | RegisterAccessError::WriteForbidden { .. } => Self::Access,
            RegisterAccessError::ReservedBitChange { .. } => Self::ReservedBits,
            RegisterAccessError::Other(error) => error,
        }
    }
}

impl From<hardware_interface_schema::RegisterAccessError> for Error {
    fn from(error: hardware_interface_schema::RegisterAccessError) -> Self {
        error.widen::<Self>().into()
    }
}

impl From<hardware_interface_pipeline::BranchError<Self>> for Error {
    fn from(error: hardware_interface_pipeline::BranchError<Self>) -> Self {
        use hardware_interface_pipeline::BranchError;
        match error {
            BranchError::Application(error) => error,
            BranchError::InvalidIndex { .. }
            | BranchError::DuplicateIndex { .. }
            | BranchError::Unavailable { .. }
            | BranchError::EmptyInput
            | BranchError::InputCount { .. }
            | BranchError::OutputCount { .. } => Self::Routing,
        }
    }
}
