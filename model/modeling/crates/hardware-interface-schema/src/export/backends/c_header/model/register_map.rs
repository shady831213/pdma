use crate::{
    register::{Access, RegisterSpec, ResetKind},
    register_map::{ResolvedRegisterBank, ResolvedRegisterMap},
};

/// One keyed register row consumed by C-header renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterCHeaderModel<I, F = u16> {
    id: I,
    name: &'static str,
    offset: usize,
    width: usize,
    access: Access,
    reset_kind: ResetKind,
    reset_value: u64,
    writable_mask: u64,
    field_id: core::marker::PhantomData<fn() -> F>,
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
            field_id: core::marker::PhantomData,
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
}

/// One complete repeated register-bank model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterCHeaderBank<I, F = u16> {
    name: &'static str,
    base: usize,
    count: usize,
    stride: usize,
    registers: Vec<RegisterCHeaderModel<I, F>>,
}

impl<I: Copy, F: Copy> RegisterCHeaderBank<I, F> {
    fn from_resolved(bank: &ResolvedRegisterBank<I, F>) -> Self {
        Self {
            name: bank.name(),
            base: bank.base(),
            count: bank.count(),
            stride: bank.stride(),
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
}

/// Complete ordered typed model for one register-map C fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterMapCHeaderModel<I, F = u16> {
    size: usize,
    fixed: Vec<RegisterCHeaderModel<I, F>>,
    banks: Vec<RegisterCHeaderBank<I, F>>,
}

impl<I: Copy + Eq + 'static, F: Copy + Eq + core::fmt::Debug> RegisterMapCHeaderModel<I, F> {
    /// Projects a resolved register map into a complete typed C fragment model.
    #[must_use]
    pub fn from_resolved(schema: &ResolvedRegisterMap<I, F>) -> Self {
        let fixed = schema
            .registers()
            .iter()
            .filter(|register| register.instance().is_none())
            .map(|register| {
                RegisterCHeaderModel::from_resolved(RegisterSpec::place(
                    register.id(),
                    register.name(),
                    register.offset(),
                    register.definition(),
                ))
            })
            .collect();
        let banks = schema
            .banks()
            .iter()
            .map(RegisterCHeaderBank::from_resolved)
            .collect();
        Self {
            size: schema.size(),
            fixed,
            banks,
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
}
