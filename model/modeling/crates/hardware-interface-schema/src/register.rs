use core::fmt;

/// Declared software access permission for a register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Reads are permitted and writes are not.
    ReadOnly,
    /// Reads and writes are permitted.
    ReadWrite,
    /// Writes are permitted and reads are not.
    WriteOnly,
}

/// Application-defined reset classification attached to a register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetKind {
    /// The declared reset value is constant.
    Constant,
    /// The application classifies the register as zero-reset.
    Zero,
    /// The application populates the register after reset.
    PostReset,
}

/// One immutable register declaration with a caller-defined typed identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterSpec<I> {
    id: I,
    numeric_id: Option<u16>,
    name: &'static str,
    offset: usize,
    width: usize,
    access: Access,
    reset_kind: ResetKind,
    reset_value: u64,
    writable_mask: u64,
}

impl<I: Copy> RegisterSpec<I> {
    /// Creates a const-friendly read-only, zero-reset register declaration.
    pub const fn new(id: I, name: &'static str, offset: usize, width: usize) -> Self {
        Self {
            id,
            numeric_id: None,
            name,
            offset,
            width,
            access: Access::ReadOnly,
            reset_kind: ResetKind::Zero,
            reset_value: 0,
            writable_mask: 0,
        }
    }

    /// Attaches the explicit numeric identity from a schema declaration.
    #[must_use]
    pub const fn with_numeric_id(mut self, id: u16) -> Self {
        self.numeric_id = Some(id);
        self
    }

    /// Replaces the declared access permission.
    #[must_use]
    pub const fn with_access(mut self, access: Access) -> Self {
        self.access = access;
        self
    }

    /// Replaces the reset classification and declared value.
    #[must_use]
    pub const fn with_reset(mut self, kind: ResetKind, value: u64) -> Self {
        self.reset_kind = kind;
        self.reset_value = value;
        self
    }

    /// Replaces the writable-bit metadata.
    #[must_use]
    pub const fn with_writable_mask(mut self, mask: u64) -> Self {
        self.writable_mask = mask;
        self
    }

    /// Returns the caller-defined identity.
    pub const fn id(self) -> I {
        self.id
    }

    /// Returns the explicit numeric schema identity, when declared.
    pub const fn numeric_id(self) -> Option<u16> {
        self.numeric_id
    }

    /// Returns the symbolic name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the map-relative or bank-relative starting offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the declared byte width.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Returns the declared access permission.
    pub const fn access(self) -> Access {
        self.access
    }

    /// Returns the application-defined reset classification.
    pub const fn reset_kind(self) -> ResetKind {
        self.reset_kind
    }

    /// Returns the declared reset value.
    pub const fn reset_value(self) -> u64 {
        self.reset_value
    }

    /// Returns the writable-bit metadata.
    pub const fn writable_mask(self) -> u64 {
        self.writable_mask
    }

    /// Validates width, writable mask, and reset value in deterministic order.
    ///
    /// # Errors
    /// Returns the first invalid property in width, writable-mask, reset-value order.
    pub const fn validate(self) -> Result<(), RegisterValidationError> {
        if !matches!(self.width, 1 | 2 | 4 | 8) {
            return Err(RegisterValidationError::InvalidWidth {
                name: self.name,
                width: self.width,
            });
        }
        if !fits_width(self.writable_mask, self.width) {
            return Err(RegisterValidationError::WritableMaskOverflow {
                name: self.name,
                width: self.width,
                mask: self.writable_mask,
            });
        }
        if !fits_width(self.reset_value, self.width) {
            return Err(RegisterValidationError::ResetValueOverflow {
                name: self.name,
                width: self.width,
                value: self.reset_value,
            });
        }
        Ok(())
    }
}

const fn fits_width(value: u64, width: usize) -> bool {
    width == 8 || value < (1_u64 << (width * 8))
}

/// Failure while validating one register declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegisterValidationError {
    /// The declared width is not 1, 2, 4, or 8 bytes.
    InvalidWidth {
        /// Symbolic register name.
        name: &'static str,
        /// Unsupported byte width.
        width: usize,
    },
    /// The writable mask does not fit the declared width.
    WritableMaskOverflow {
        /// Symbolic register name.
        name: &'static str,
        /// Declared byte width.
        width: usize,
        /// Supplied mask.
        mask: u64,
    },
    /// The reset value does not fit the declared width.
    ResetValueOverflow {
        /// Symbolic register name.
        name: &'static str,
        /// Declared byte width.
        width: usize,
        /// Supplied reset value.
        value: u64,
    },
}

impl fmt::Display for RegisterValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid register schema: {self:?}")
    }
}

impl std::error::Error for RegisterValidationError {}
