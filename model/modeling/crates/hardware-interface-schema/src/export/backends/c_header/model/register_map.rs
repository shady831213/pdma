use crate::{
    BitsFieldSpec, Documentation,
    register::{Access, RegisterSpec, ResetKind},
    register_map::{ResolvedRegisterBank, ResolvedRegisterMap},
};

/// One keyed register row consumed by C-header renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterCHeaderModel<I, F: 'static = u16> {
    id: I,
    name: &'static str,
    offset: usize,
    width: usize,
    access: Access,
    reset_kind: ResetKind,
    reset_value: u64,
    writable_mask: u64,
    fields: &'static [BitsFieldSpec<F>],
    documentation: Documentation,
}

impl<I: Copy, F: Copy> RegisterCHeaderModel<I, F> {
    const fn from_resolved(register: RegisterSpec<I, F>) -> Self {
        Self {
            id: register.id(),
            name: register.name(),
            offset: register.offset(),
            width: register.width(),
            access: register.access(),
            reset_kind: register.reset_kind(),
            reset_value: register.reset_value(),
            writable_mask: register.writable_mask(),
            fields: register.definition().fields(),
            documentation: register.documentation(),
        }
    }

    /// Returns the typed register identity.
    pub const fn id(self) -> I {
        self.id
    }

    /// Returns the register name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the map- or bank-relative offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the register width.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Returns the access class.
    pub const fn access(self) -> Access {
        self.access
    }

    /// Returns the reset class.
    pub const fn reset_kind(self) -> ResetKind {
        self.reset_kind
    }

    /// Returns the reset value.
    pub const fn reset_value(self) -> u64 {
        self.reset_value
    }

    /// Returns the writable-bit mask.
    pub const fn writable_mask(self) -> u64 {
        self.writable_mask
    }

    /// Returns the register's effective description text.
    pub const fn documentation(self) -> Documentation {
        self.documentation
    }

    /// Returns the declared bit fields and their descriptions.
    pub const fn fields(self) -> &'static [BitsFieldSpec<F>] {
        self.fields
    }
}

/// One complete repeated register-bank model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterCHeaderBank<I, F: 'static = u16> {
    path: String,
    name: &'static str,
    base: usize,
    count: usize,
    stride: usize,
    registers: Vec<RegisterCHeaderModel<I, F>>,
    documentation: Documentation,
}

impl<I: Copy, F: Copy> RegisterCHeaderBank<I, F> {
    fn from_resolved(bank: &ResolvedRegisterBank<I, F>) -> Self {
        Self {
            path: bank.path().to_owned(),
            name: bank.name(),
            base: bank.base(),
            count: bank.count(),
            stride: bank.stride(),
            documentation: bank.documentation(),
            registers: bank
                .registers()
                .iter()
                .copied()
                .map(RegisterCHeaderModel::from_resolved)
                .collect(),
        }
    }

    /// Returns the bank name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the owned bank declaration's root-qualified path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the first instance base.
    pub const fn base(&self) -> usize {
        self.base
    }

    /// Returns the instance count.
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns the instance stride.
    pub const fn stride(&self) -> usize {
        self.stride
    }

    /// Returns every bank-relative register row.
    pub fn registers(&self) -> &[RegisterCHeaderModel<I, F>] {
        &self.registers
    }

    /// Returns the bank's description text.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }
}

/// Complete ordered typed model for one register-map C fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterMapCHeaderModel<I, F: 'static = u16> {
    expanded_banks: bool,
    size: usize,
    fixed: Vec<RegisterCHeaderModel<I, F>>,
    banks: Vec<RegisterCHeaderBank<I, F>>,
    documentation: Documentation,
    groups: Vec<(String, Documentation)>,
}

impl<I: Copy + Eq + 'static, F: Copy + Eq + core::fmt::Debug> RegisterMapCHeaderModel<I, F> {
    /// Projects a resolved register map into a complete typed C fragment model.
    #[must_use]
    pub fn from_resolved(schema: &ResolvedRegisterMap<I, F>) -> Self {
        let expanded_banks = schema.banks().iter().any(|bank| bank.ordered);
        let fixed = schema
            .registers()
            .iter()
            .filter(|register| {
                expanded_banks
                    || register.instance().is_none()
                    || !schema.banks().iter().any(|bank| {
                        bank.expanded_registers()
                            .iter()
                            .any(|member| member.path() == register.path())
                    })
            })
            .map(|register| {
                RegisterCHeaderModel::from_resolved(
                    RegisterSpec::place(
                        register.id(),
                        register.name(),
                        register.offset(),
                        register.definition(),
                    )
                    .with_documentation(register.documentation()),
                )
            })
            .collect();
        let banks = schema
            .banks()
            .iter()
            .map(RegisterCHeaderBank::from_resolved)
            .collect();
        Self {
            expanded_banks,
            size: schema.size(),
            fixed,
            banks,
            documentation: schema.documentation(),
            groups: schema
                .placements()
                .iter()
                .filter(|node| !node.placement().is_bank())
                .map(|placement| (placement.path().into(), placement.documentation()))
                .collect(),
        }
    }

    /// Returns the register-map extent.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns all fixed and fixed-group rows in canonical order.
    pub fn fixed(&self) -> &[RegisterCHeaderModel<I, F>] {
        &self.fixed
    }

    /// Returns all repeated bank models in canonical order.
    pub fn banks(&self) -> &[RegisterCHeaderBank<I, F>] {
        &self.banks
    }

    /// Returns the root map's description text.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }

    /// Returns nested map paths and their instance descriptions.
    pub fn groups(&self) -> &[(String, Documentation)] {
        &self.groups
    }

    pub(crate) const fn expanded_banks(&self) -> bool {
        self.expanded_banks
    }
}
