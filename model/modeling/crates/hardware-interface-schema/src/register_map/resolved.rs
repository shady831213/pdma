use crate::register::{RegisterDefinition, RegisterSpec};

/// One fixed or repeated register instance with a deterministic path and absolute offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRegister<I: 'static = u16, F: 'static = u16> {
    pub(super) path: String,
    pub(super) offset: usize,
    pub(super) declaration_index: usize,
    pub(super) register: RegisterSpec<I, F>,
    pub(super) bank_name: Option<&'static str>,
    pub(super) instance: Option<usize>,
}

impl<I: Copy, F: Copy + Eq + core::fmt::Debug> ResolvedRegister<I, F> {
    /// Returns the root-qualified instance path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the absolute map-relative byte offset.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the placement declaration index.
    pub const fn declaration_index(&self) -> usize {
        self.declaration_index
    }

    /// Returns the placement identity.
    pub const fn id(&self) -> I {
        self.register.id()
    }

    /// Returns the placement name.
    pub const fn name(&self) -> &'static str {
        self.register.name()
    }

    /// Returns the reused offset-free definition.
    pub const fn definition(&self) -> RegisterDefinition<F> {
        self.register.definition()
    }

    /// Returns the bank name for a repeated instance.
    pub const fn bank_name(&self) -> Option<&'static str> {
        self.bank_name
    }

    /// Returns the bank instance index for a repeated instance.
    pub const fn instance(&self) -> Option<usize> {
        self.instance
    }
}

/// Fully validated register map in deterministic declaration and instance order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRegisterMap<I: 'static = u16, F: 'static = u16> {
    pub(super) root: &'static str,
    pub(super) size: usize,
    pub(super) registers: Vec<ResolvedRegister<I, F>>,
}

impl<I: Copy, F: Copy + Eq + core::fmt::Debug> ResolvedRegisterMap<I, F> {
    /// Returns the root map name.
    pub const fn root(&self) -> &'static str {
        self.root
    }

    /// Returns the map extent.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns resolved instances in deterministic order.
    pub fn registers(&self) -> &[ResolvedRegister<I, F>] {
        &self.registers
    }

    /// Finds a register only at its exact expanded start.
    pub fn locate(&self, offset: usize) -> Option<&ResolvedRegister<I, F>> {
        self.registers
            .iter()
            .find(|register| register.offset == offset)
    }
}
