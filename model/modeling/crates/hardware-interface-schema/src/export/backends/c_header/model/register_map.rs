use crate::{
    register::{Access, RegisterSpec, ResetKind},
    register_map::{RegisterMapSpec, RepeatedRegisterBank},
};

use super::{FragmentModelError, identity::validate_rows};

/// One keyed register row consumed by C-header renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterCHeaderModel<I> {
    id: I,
    name: &'static str,
    offset: usize,
    width: usize,
    access: Access,
    reset_kind: ResetKind,
    reset_value: u64,
    writable_mask: u64,
}

impl<I: Copy> RegisterCHeaderModel<I> {
    /// Projects one register schema into typed C-header metadata.
    #[must_use]
    pub const fn from_spec(register: RegisterSpec<I>) -> Self {
        Self {
            id: register.id(),
            name: register.name(),
            offset: register.offset(),
            width: register.width(),
            access: register.access(),
            reset_kind: register.reset_kind(),
            reset_value: register.reset_value(),
            writable_mask: register.writable_mask(),
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
pub struct RegisterCHeaderBank<I> {
    name: &'static str,
    base: usize,
    count: usize,
    stride: usize,
    registers: Vec<RegisterCHeaderModel<I>>,
}

impl<I: Copy> RegisterCHeaderBank<I> {
    fn from_spec(bank: RepeatedRegisterBank<I>) -> Self {
        Self {
            name: bank.name(),
            base: bank.base(),
            count: bank.count(),
            stride: bank.stride(),
            registers: bank
                .registers()
                .iter()
                .copied()
                .map(RegisterCHeaderModel::from_spec)
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
    pub fn registers(&self) -> &[RegisterCHeaderModel<I>] {
        &self.registers
    }
}

/// Complete ordered typed model for one register-map C fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterMapCHeaderModel<I> {
    size: usize,
    fixed: Vec<RegisterCHeaderModel<I>>,
    banks: Vec<RegisterCHeaderBank<I>>,
}

impl<I: Copy + Eq + 'static> RegisterMapCHeaderModel<I> {
    /// Projects a register map into a complete typed C fragment model.
    #[must_use]
    pub fn from_spec(schema: RegisterMapSpec<I>) -> Self {
        let fixed = schema
            .registers()
            .iter()
            .chain(
                schema
                    .register_groups()
                    .iter()
                    .flat_map(|group| group.iter()),
            )
            .copied()
            .map(RegisterCHeaderModel::from_spec)
            .collect();
        let banks = schema
            .banks()
            .iter()
            .copied()
            .map(RegisterCHeaderBank::from_spec)
            .collect();
        Self {
            size: schema.size(),
            fixed,
            banks,
        }
    }

    /// Validates caller-supplied rows against their source map.
    ///
    /// # Errors
    /// Returns the first duplicate, unknown, missing, reordered, or mismatched row.
    pub fn try_from_parts(
        schema: RegisterMapSpec<I>,
        fixed: Vec<RegisterCHeaderModel<I>>,
        banks: Vec<RegisterCHeaderBank<I>>,
    ) -> Result<Self, FragmentModelError> {
        let expected = Self::from_spec(schema);
        validate_rows(&expected.fixed, &fixed, |register| register.id())?;
        let expected_bank_names: Vec<_> = expected
            .banks
            .iter()
            .map(RegisterCHeaderBank::name)
            .collect();
        let actual_bank_names: Vec<_> = banks.iter().map(RegisterCHeaderBank::name).collect();
        validate_rows(&expected_bank_names, &actual_bank_names, |name| *name)?;
        for (index, (expected_bank, actual_bank)) in expected.banks.iter().zip(&banks).enumerate() {
            if expected_bank.base != actual_bank.base
                || expected_bank.count != actual_bank.count
                || expected_bank.stride != actual_bank.stride
            {
                return Err(FragmentModelError::MismatchedRow { index });
            }
            validate_rows(
                expected_bank.registers(),
                actual_bank.registers(),
                |register| register.id(),
            )?;
        }
        Ok(Self {
            size: expected.size,
            fixed,
            banks,
        })
    }

    /// Returns the register-map extent.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns all fixed and fixed-group rows in canonical order.
    pub fn fixed(&self) -> &[RegisterCHeaderModel<I>] {
        &self.fixed
    }

    /// Returns all repeated bank models in canonical order.
    pub fn banks(&self) -> &[RegisterCHeaderBank<I>] {
        &self.banks
    }
}

#[cfg(test)]
mod tests;
