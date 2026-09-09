use super::ResolvedRegisterMapPlacement;
use crate::Documentation;
use crate::register::{RegisterDefinition, RegisterSpec};

/// One resolved repeated-bank declaration and its ordered template members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRegisterBank<I: 'static = u16, F: 'static = u16> {
    pub(super) documentation: Documentation,
    pub(super) path: String,
    pub(super) name: &'static str,
    pub(super) declaration_index: usize,
    pub(super) base: usize,
    pub(super) count: usize,
    pub(super) stride: usize,
    pub(super) registers: Vec<RegisterSpec<I, F>>,
    pub(super) expanded_registers: Vec<ResolvedRegister<I, F>>,
}

impl<I: Copy, F: Copy> ResolvedRegisterBank<I, F> {
    /// Returns the bank documentation.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }

    /// Returns the root-qualified bank path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the declared bank name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the first member's declaration index.
    pub const fn declaration_index(&self) -> usize {
        self.declaration_index
    }

    /// Returns the first instance base.
    pub const fn base(&self) -> usize {
        self.base
    }

    /// Returns the instance count.
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns the distance between instance bases.
    pub const fn stride(&self) -> usize {
        self.stride
    }

    /// Returns template members in declaration order.
    pub fn registers(&self) -> &[RegisterSpec<I, F>] {
        &self.registers
    }

    /// Returns expanded instances in declaration and instance order.
    pub fn expanded_registers(&self) -> &[ResolvedRegister<I, F>] {
        &self.expanded_registers
    }
}

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
    /// Returns placement documentation with definition fallback.
    pub const fn documentation(&self) -> Documentation {
        self.register.documentation()
    }

    pub(crate) fn ordered(
        path: String,
        offset: usize,
        declaration_index: usize,
        register: RegisterSpec<I, F>,
        repeated: Option<(&'static str, usize)>,
    ) -> Self {
        let (bank_name, instance) = repeated.map_or((None, None), |(name, instance)| {
            (Some(name), Some(instance))
        });
        Self {
            path,
            offset,
            declaration_index,
            register,
            bank_name,
            instance,
        }
    }

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
    pub(super) placements: Vec<ResolvedRegisterMapPlacement<I>>,
    pub(super) documentation: Documentation,
    pub(super) root: &'static str,
    pub(super) size: usize,
    pub(super) registers: Vec<ResolvedRegister<I, F>>,
    pub(super) banks: Vec<ResolvedRegisterBank<I, F>>,
}

impl<I: Copy, F: Copy + Eq + core::fmt::Debug> ResolvedRegisterMap<I, F> {
    /// Attaches nested map instances in depth-first declaration order.
    #[must_use]
    pub fn with_placements(mut self, placements: Vec<ResolvedRegisterMapPlacement<I>>) -> Self {
        self.placements = placements;
        self
    }

    /// Returns nested map instances in depth-first declaration order.
    pub fn placements(&self) -> &[ResolvedRegisterMapPlacement<I>] {
        &self.placements
    }

    /// Attaches documentation to this node.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns documentation of this node.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }

    pub(crate) const fn ordered(
        root: &'static str,
        size: usize,
        registers: Vec<ResolvedRegister<I, F>>,
    ) -> Self {
        Self {
            root,
            size,
            registers,
            banks: Vec::new(),
            placements: Vec::new(),
            documentation: Documentation::EMPTY,
        }
    }

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

    /// Returns repeated banks in declaration order.
    pub fn banks(&self) -> &[ResolvedRegisterBank<I, F>] {
        &self.banks
    }

    /// Finds a register only at its exact expanded start.
    pub fn locate(&self, offset: usize) -> Option<&ResolvedRegister<I, F>> {
        self.registers
            .iter()
            .find(|register| register.offset == offset)
    }
}
