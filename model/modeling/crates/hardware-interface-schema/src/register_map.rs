mod error;
mod validation;

pub use error::RegisterMapValidationError;

use crate::register::RegisterSpec;

/// One immutable repeated register-bank declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatedRegisterBank<I: 'static> {
    name: &'static str,
    base: usize,
    count: usize,
    stride: usize,
    registers: &'static [RegisterSpec<I>],
}

impl<I: Copy> RepeatedRegisterBank<I> {
    /// Creates a bank whose register offsets are relative to each instance base.
    pub const fn new(
        name: &'static str,
        base: usize,
        count: usize,
        stride: usize,
        registers: &'static [RegisterSpec<I>],
    ) -> Self {
        Self {
            name,
            base,
            count,
            stride,
            registers,
        }
    }

    /// Returns the bank name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the first instance's map-relative base.
    pub const fn base(self) -> usize {
        self.base
    }

    /// Returns the number of instances.
    pub const fn count(self) -> usize {
        self.count
    }

    /// Returns the byte distance between instance bases.
    pub const fn stride(self) -> usize {
        self.stride
    }

    /// Returns the bank-relative register declarations.
    pub const fn registers(self) -> &'static [RegisterSpec<I>] {
        self.registers
    }
}

/// A typed exact-start lookup result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterLocation<I> {
    /// A fixed map-relative register.
    Fixed {
        /// Register declaration.
        register: RegisterSpec<I>,
        /// Expanded map-relative start.
        offset: usize,
    },
    /// A member of one repeated-bank instance.
    Repeated {
        /// Repeated-bank name.
        bank_name: &'static str,
        /// Zero-based bank instance.
        instance: usize,
        /// Bank-relative register declaration.
        register: RegisterSpec<I>,
        /// Expanded map-relative start.
        offset: usize,
    },
}

/// An immutable map extent, fixed registers, and repeated banks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterMapSpec<I: 'static> {
    size: usize,
    registers: &'static [RegisterSpec<I>],
    register_groups: &'static [&'static [RegisterSpec<I>]],
    banks: &'static [RepeatedRegisterBank<I>],
}

impl<I: Copy + Eq + 'static> RegisterMapSpec<I> {
    /// Creates an unvalidated const-friendly register map.
    pub const fn new(
        size: usize,
        registers: &'static [RegisterSpec<I>],
        banks: &'static [RepeatedRegisterBank<I>],
    ) -> Self {
        Self {
            size,
            registers,
            register_groups: &[],
            banks,
        }
    }

    /// Adds independently declared fixed-register groups to the map.
    #[must_use]
    pub const fn with_register_groups(
        mut self,
        groups: &'static [&'static [RegisterSpec<I>]],
    ) -> Self {
        self.register_groups = groups;
        self
    }

    /// Returns the map extent in bytes.
    pub const fn size(self) -> usize {
        self.size
    }

    /// Returns fixed map-relative register declarations.
    pub const fn registers(self) -> &'static [RegisterSpec<I>] {
        self.registers
    }

    /// Returns independently declared fixed-register groups.
    pub const fn register_groups(self) -> &'static [&'static [RegisterSpec<I>]] {
        self.register_groups
    }

    /// Returns repeated-bank declarations.
    pub const fn banks(self) -> &'static [RepeatedRegisterBank<I>] {
        self.banks
    }

    /// Finds a register only when `offset` is its exact expanded start.
    #[must_use]
    pub fn locate(self, offset: usize) -> Option<RegisterLocation<I>> {
        if let Some(register) = self
            .registers
            .iter()
            .copied()
            .find(|register| register.offset() == offset)
        {
            return Some(RegisterLocation::Fixed { register, offset });
        }
        if let Some(register) = self
            .register_groups
            .iter()
            .flat_map(|group| group.iter())
            .copied()
            .find(|register| register.offset() == offset)
        {
            return Some(RegisterLocation::Fixed { register, offset });
        }
        for bank in self.banks.iter().copied() {
            if bank.stride == 0 || offset < bank.base {
                continue;
            }
            let relative = offset - bank.base;
            let instance = relative / bank.stride;
            if instance >= bank.count {
                continue;
            }
            let member_offset = relative % bank.stride;
            if let Some(register) = bank
                .registers
                .iter()
                .copied()
                .find(|register| register.offset() == member_offset)
            {
                return Some(RegisterLocation::Repeated {
                    bank_name: bank.name,
                    instance,
                    register,
                    offset,
                });
            }
        }
        None
    }
}
