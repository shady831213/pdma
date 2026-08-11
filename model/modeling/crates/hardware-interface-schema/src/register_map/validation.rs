use crate::register::RegisterSpec;

use super::{RegisterMapSpec, RegisterMapValidationError};

impl<I: Copy + Eq + 'static> RegisterMapSpec<I> {
    /// Validates declarations and expanded layout in deterministic pass order.
    ///
    /// Duplicate IDs, names, and expanded starts precede register scalar metadata.
    /// Map extents and bank stride rules precede final expanded-range overlap.
    ///
    /// # Errors
    /// Returns the first error in the documented pass order and declaration order.
    pub fn validate(self) -> Result<(), RegisterMapValidationError<I>> {
        self.validate_duplicates()?;
        self.validate_duplicate_starts()?;
        for register in self.declarations() {
            register
                .validate()
                .map_err(RegisterMapValidationError::Register)?;
        }
        self.validate_layout()?;
        self.validate_overlaps()
    }

    fn declarations(self) -> impl Iterator<Item = RegisterSpec<I>> {
        self.registers
            .iter()
            .copied()
            .chain(
                self.register_groups
                    .iter()
                    .flat_map(|group| group.iter().copied()),
            )
            .chain(
                self.banks
                    .iter()
                    .flat_map(|bank| bank.registers.iter().copied()),
            )
    }

    fn validate_duplicates(self) -> Result<(), RegisterMapValidationError<I>> {
        for (index, register) in self.declarations().enumerate() {
            for previous in self.declarations().take(index) {
                let numeric_id_matches = previous.numeric_id().is_some()
                    && previous.numeric_id() == register.numeric_id();
                if previous.id() == register.id() || numeric_id_matches {
                    return Err(RegisterMapValidationError::DuplicateRegisterId {
                        id: register.id(),
                    });
                }
            }
        }
        for (index, register) in self.declarations().enumerate() {
            for previous in self.declarations().take(index) {
                if previous.name() == register.name() {
                    return Err(RegisterMapValidationError::DuplicateRegisterName {
                        name: register.name(),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_duplicate_starts(self) -> Result<(), RegisterMapValidationError<I>> {
        let Some(count) = self.expanded_count() else {
            return Ok(());
        };
        for index in 0..count {
            let Some(register) = self.expanded(index) else {
                continue;
            };
            for previous_index in 0..index {
                if self
                    .expanded(previous_index)
                    .is_some_and(|previous| previous.offset == register.offset)
                {
                    return Err(RegisterMapValidationError::DuplicateExpandedStart {
                        offset: register.offset,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_layout(self) -> Result<(), RegisterMapValidationError<I>> {
        for register in self.registers.iter().copied() {
            self.check_extent(register.name(), register.offset(), register.width())?;
        }
        for register in self
            .register_groups
            .iter()
            .flat_map(|group| group.iter().copied())
        {
            self.check_extent(register.name(), register.offset(), register.width())?;
        }
        for bank in self.banks.iter().copied() {
            if bank.stride == 0 {
                return Err(RegisterMapValidationError::BankStrideZero {
                    bank_name: bank.name,
                });
            }
            let alignment = bank
                .registers
                .iter()
                .fold(1, |widest, register| widest.max(register.width()));
            if bank.stride % alignment != 0 {
                return Err(RegisterMapValidationError::BankStrideMisaligned {
                    bank_name: bank.name,
                    stride: bank.stride,
                    required_alignment: alignment,
                });
            }
            let extent = bank.registers.iter().fold(0, |largest, register| {
                register
                    .offset()
                    .checked_add(register.width())
                    .map_or(usize::MAX, |end| largest.max(end))
            });
            if bank.stride < extent {
                return Err(RegisterMapValidationError::BankStrideTooSmall {
                    bank_name: bank.name,
                    stride: bank.stride,
                    required_extent: extent,
                });
            }
            if bank.count > 0 {
                self.validate_last_bank_instance(bank)?;
            }
        }
        Ok(())
    }

    fn validate_last_bank_instance(
        self,
        bank: super::RepeatedRegisterBank<I>,
    ) -> Result<(), RegisterMapValidationError<I>> {
        let instance = bank.count - 1;
        for register in bank.registers.iter().copied() {
            let offset = instance
                .checked_mul(bank.stride)
                .and_then(|value| bank.base.checked_add(value))
                .and_then(|value| value.checked_add(register.offset()))
                .ok_or_else(|| RegisterMapValidationError::BankExpansionOverflow {
                    bank_name: bank.name,
                    instance,
                    register_name: register.name(),
                })?;
            self.check_extent(register.name(), offset, register.width())?;
        }
        Ok(())
    }

    fn check_extent(
        self,
        name: &'static str,
        offset: usize,
        width: usize,
    ) -> Result<(), RegisterMapValidationError<I>> {
        if offset.checked_add(width).is_none_or(|end| end > self.size) {
            return Err(RegisterMapValidationError::RegisterOutOfBounds {
                name,
                offset,
                width,
                map_size: self.size,
            });
        }
        Ok(())
    }

    fn validate_overlaps(self) -> Result<(), RegisterMapValidationError<I>> {
        let Some(count) = self.expanded_count() else {
            return Ok(());
        };
        for index in 0..count {
            let Some(register) = self.expanded(index) else {
                continue;
            };
            for previous_index in 0..index {
                let Some(previous) = self.expanded(previous_index) else {
                    continue;
                };
                if previous.offset < register.offset.saturating_add(register.register.width())
                    && register.offset < previous.offset.saturating_add(previous.register.width())
                {
                    return Err(RegisterMapValidationError::ExpandedOverlap {
                        first_name: previous.register.name(),
                        first_instance: previous.instance,
                        second_name: register.register.name(),
                        second_instance: register.instance,
                    });
                }
            }
        }
        Ok(())
    }

    fn expanded_count(self) -> Option<usize> {
        self.banks
            .iter()
            .try_fold(self.fixed_register_count()?, |count, bank| {
                bank.count
                    .checked_mul(bank.registers.len())
                    .and_then(|bank_count| count.checked_add(bank_count))
            })
    }

    fn expanded(self, index: usize) -> Option<Expanded<I>> {
        if let Some(register) = self.fixed_register(index) {
            return Some(Expanded {
                register,
                instance: None,
                offset: register.offset(),
            });
        }
        let mut relative = index.checked_sub(self.fixed_register_count()?)?;
        for bank in self.banks.iter().copied() {
            let bank_count = bank.count.checked_mul(bank.registers.len())?;
            if relative < bank_count {
                let instance = relative / bank.registers.len();
                let register = bank
                    .registers
                    .get(relative % bank.registers.len())
                    .copied()?;
                let offset = instance
                    .checked_mul(bank.stride)?
                    .checked_add(bank.base)?
                    .checked_add(register.offset())?;
                return Some(Expanded {
                    register,
                    instance: Some(instance),
                    offset,
                });
            }
            relative -= bank_count;
        }
        None
    }

    fn fixed_register_count(self) -> Option<usize> {
        self.register_groups
            .iter()
            .try_fold(self.registers.len(), |count, group| {
                count.checked_add(group.len())
            })
    }

    fn fixed_register(self, index: usize) -> Option<RegisterSpec<I>> {
        if let Some(register) = self.registers.get(index).copied() {
            return Some(register);
        }
        let mut relative = index.checked_sub(self.registers.len())?;
        for group in self.register_groups {
            if let Some(register) = group.get(relative).copied() {
                return Some(register);
            }
            relative = relative.checked_sub(group.len())?;
        }
        None
    }
}

#[derive(Clone, Copy)]
struct Expanded<I: 'static> {
    register: RegisterSpec<I>,
    instance: Option<usize>,
    offset: usize,
}
