use std::convert::Infallible;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Base address of one numeric register-bus window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterBusBase(u64);

impl RegisterBusBase {
    /// Creates a base from its full-width numeric representation.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the full-width numeric representation.
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Full address presented to one numeric register-bus window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterBusAddress(u64);

impl RegisterBusAddress {
    /// Creates an address from its full-width numeric representation.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the full-width numeric representation.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Translates this full address into the canonical offset coordinate space.
    pub fn offset_from(
        self,
        base: RegisterBusBase,
        width: RegisterBusWidth,
    ) -> Result<RegisterBusOffset, RegisterBusError> {
        RegisterBusOffset::from_address(base, self, width)
    }
}

/// Canonical numeric register offset after checked address translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterBusOffset(u32);

impl RegisterBusOffset {
    /// Translates one full address by subtracting its base before checked narrowing.
    pub fn from_address(
        base: RegisterBusBase,
        address: RegisterBusAddress,
        width: RegisterBusWidth,
    ) -> Result<Self, RegisterBusError> {
        let delta =
            address
                .raw()
                .checked_sub(base.raw())
                .ok_or(RegisterBusError::AddressBelowBase {
                    base,
                    address,
                    width,
                })?;
        u32::try_from(delta)
            .map(Self)
            .map_err(|_| RegisterBusError::OffsetOutOfRange {
                base,
                address,
                width,
                delta,
            })
    }

    /// Returns the canonical numeric offset.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Exact caller-provided register-bus access width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterBusWidth(usize);

impl RegisterBusWidth {
    /// Creates a width without narrowing its caller-provided representation.
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the exact caller-provided width.
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Address-translation or endpoint failure for one addressed register-bus access.
#[derive(Clone, Copy, Debug)]
pub enum RegisterBusError<E = Infallible> {
    /// The full address was below the supplied window base.
    AddressBelowBase {
        /// Supplied full-width window base.
        base: RegisterBusBase,
        /// Supplied full-width address.
        address: RegisterBusAddress,
        /// Exact caller-provided access width.
        width: RegisterBusWidth,
    },
    /// The post-subtraction full-width delta cannot fit the canonical offset.
    OffsetOutOfRange {
        /// Supplied full-width window base.
        base: RegisterBusBase,
        /// Supplied full-width address.
        address: RegisterBusAddress,
        /// Exact caller-provided access width.
        width: RegisterBusWidth,
        /// Full-width post-subtraction delta that could not narrow.
        delta: u64,
    },
    /// The translated endpoint rejected the access after successful translation.
    Endpoint {
        /// Supplied full-width window base.
        base: RegisterBusBase,
        /// Supplied full-width address.
        address: RegisterBusAddress,
        /// Exact caller-provided access width.
        width: RegisterBusWidth,
        /// Endpoint-owned failure.
        source: E,
    },
}

impl<E> RegisterBusError<E> {
    pub(crate) const fn from_translation(error: RegisterBusError) -> Self {
        match error {
            RegisterBusError::AddressBelowBase {
                base,
                address,
                width,
            } => Self::AddressBelowBase {
                base,
                address,
                width,
            },
            RegisterBusError::OffsetOutOfRange {
                base,
                address,
                width,
                delta,
            } => Self::OffsetOutOfRange {
                base,
                address,
                width,
                delta,
            },
            RegisterBusError::Endpoint { source, .. } => match source {},
        }
    }
}

impl<E: Display> Display for RegisterBusError<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressBelowBase {
                base,
                address,
                width,
            } => write!(
                formatter,
                "register address {:#x} is below base {:#x} for width {}",
                address.raw(),
                base.raw(),
                width.raw()
            ),
            Self::OffsetOutOfRange {
                base,
                address,
                width,
                delta,
            } => write!(
                formatter,
                "register address {:#x} from base {:#x} has unrepresentable offset {delta:#x} for width {}",
                address.raw(),
                base.raw(),
                width.raw()
            ),
            Self::Endpoint { source, .. } => Display::fmt(source, formatter),
        }
    }
}

impl<E: Error + 'static> Error for RegisterBusError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AddressBelowBase { .. } | Self::OffsetOutOfRange { .. } => None,
            Self::Endpoint { source, .. } => Some(source),
        }
    }
}
