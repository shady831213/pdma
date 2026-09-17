mod access;
mod error;
mod layout;
mod ordered;
mod placement;
mod projection;
mod resolve;
mod resolved;
mod validation;

pub use crate::register::RegisterSpec;
pub use access::{RegisterAccessGroup, RegisterAccessLeaf, RegisterAccessPath};
pub use error::{RegisterMapError, RegisterMapValidationError};
pub use ordered::{
    OrderedRegisterMapSpec, RegisterCollection, RegisterLayoutGeometry, RegisterMapPlacement,
    append_register_leaf, finish_ordered_register_map,
};
pub use placement::ResolvedRegisterMapPlacement;
pub use projection::AccessProjectionError;
pub use resolved::{ResolvedRegister, ResolvedRegisterBank, ResolvedRegisterMap};

/// One immutable repeated register-bank declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatedRegisterBank<I: 'static, F: 'static = u16> {
    documentation: crate::Documentation,
    name: &'static str,
    base: usize,
    count: usize,
    stride: usize,
    registers: &'static [RegisterSpec<I, F>],
}

impl<I: Copy, F: Copy> RepeatedRegisterBank<I, F> {
    /// Creates a bank whose register offsets are relative to each instance base.
    pub const fn new(
        name: &'static str,
        base: usize,
        count: usize,
        stride: usize,
        registers: &'static [RegisterSpec<I, F>],
    ) -> Self {
        Self {
            name,
            documentation: crate::Documentation::EMPTY,
            base,
            count,
            stride,
            registers,
        }
    }

    /// Attaches documentation to this node.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: crate::Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns documentation of this node.
    pub const fn documentation(self) -> crate::Documentation {
        self.documentation
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

    /// Returns the bank-relative register placements.
    pub const fn registers(self) -> &'static [RegisterSpec<I, F>] {
        self.registers
    }
}

/// A typed exact-start lookup result for the legacy metadata surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterLocation<I: 'static, F: 'static = u16> {
    /// A fixed map-relative register.
    Fixed {
        /// Register placement.
        register: RegisterSpec<I, F>,
        /// Expanded map-relative start.
        offset: usize,
    },
    /// A member of one repeated-bank instance.
    Repeated {
        /// Repeated-bank name.
        bank_name: &'static str,
        /// Zero-based bank instance.
        instance: usize,
        /// Bank-relative register placement.
        register: RegisterSpec<I, F>,
        /// Expanded map-relative start.
        offset: usize,
    },
}

/// An immutable named map extent, fixed placements, and repeated banks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterMapSpec<I: 'static, F: 'static = u16> {
    documentation: crate::Documentation,
    name: &'static str,
    size: usize,
    registers: &'static [RegisterSpec<I, F>],
    register_groups: &'static [&'static [RegisterSpec<I, F>]],
    banks: &'static [RepeatedRegisterBank<I, F>],
}

impl<I: Copy + 'static, F: Copy + 'static> RegisterMapSpec<I, F> {
    /// Creates an unnamed legacy-compatible map.
    pub const fn new(
        size: usize,
        registers: &'static [RegisterSpec<I, F>],
        banks: &'static [RepeatedRegisterBank<I, F>],
    ) -> Self {
        Self::named("", size, registers, banks)
    }

    /// Creates a named map suitable for deterministic resolution.
    pub const fn named(
        name: &'static str,
        size: usize,
        registers: &'static [RegisterSpec<I, F>],
        banks: &'static [RepeatedRegisterBank<I, F>],
    ) -> Self {
        Self {
            name,
            size,
            registers,
            register_groups: &[],
            documentation: crate::Documentation::EMPTY,
            banks,
        }
    }

    /// Attaches documentation to this node.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: crate::Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns documentation of this node.
    pub const fn documentation(self) -> crate::Documentation {
        self.documentation
    }

    /// Adds independently declared fixed-register groups.
    #[must_use]
    pub const fn with_register_groups(
        mut self,
        groups: &'static [&'static [RegisterSpec<I, F>]],
    ) -> Self {
        self.register_groups = groups;
        self
    }

    /// Returns the map name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the map extent in bytes.
    pub const fn size(self) -> usize {
        self.size
    }

    /// Returns fixed map-relative placements.
    pub const fn registers(self) -> &'static [RegisterSpec<I, F>] {
        self.registers
    }

    /// Returns independently declared fixed-register groups.
    pub const fn register_groups(self) -> &'static [&'static [RegisterSpec<I, F>]] {
        self.register_groups
    }

    /// Returns repeated-bank declarations.
    pub const fn banks(self) -> &'static [RepeatedRegisterBank<I, F>] {
        self.banks
    }

    /// Finds a register only at its exact expanded start.
    pub fn locate(self, offset: usize) -> Option<RegisterLocation<I, F>> {
        if let Some(register) = self.fixed().find(|register| register.offset() == offset) {
            return Some(RegisterLocation::Fixed { register, offset });
        }
        for bank in self.banks.iter().copied() {
            if bank.stride() == 0 || offset < bank.base() {
                continue;
            }
            let relative = offset - bank.base();
            let instance = relative / bank.stride();
            if instance >= bank.count() {
                continue;
            }
            let member_offset = relative % bank.stride();
            if let Some(register) = bank
                .registers()
                .iter()
                .copied()
                .find(|register| register.offset() == member_offset)
            {
                return Some(RegisterLocation::Repeated {
                    bank_name: bank.name(),
                    instance,
                    register,
                    offset,
                });
            }
        }
        None
    }

    pub(super) fn fixed(self) -> impl Iterator<Item = RegisterSpec<I, F>> {
        self.registers.iter().copied().chain(
            self.register_groups
                .iter()
                .flat_map(|group| group.iter().copied()),
        )
    }
}

impl<I: Copy + Eq + core::fmt::Debug, F: Copy + Eq + core::fmt::Debug> RegisterMapSpec<I, F> {
    /// Validates and resolves every fixed and repeated register instance.
    pub fn resolve(self) -> Result<ResolvedRegisterMap<I, F>, RegisterMapError<I, F>> {
        resolve::resolve(self)
    }
}
