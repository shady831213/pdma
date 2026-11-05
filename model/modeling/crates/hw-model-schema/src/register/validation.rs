mod fields;

use core::fmt;

use super::{
    RegisterDefinition, RegisterDefinitionError, RegisterFieldSpec, RegisterValidationError,
};

pub(super) const fn validate_legacy(
    name: &'static str,
    width: usize,
    writable_mask: u64,
    reset_value: u64,
) -> Result<(), RegisterValidationError> {
    if !supported_width(width) {
        return Err(RegisterValidationError::InvalidWidth { name, width });
    }
    if !fits_width(writable_mask, width) {
        return Err(RegisterValidationError::WritableMaskOverflow {
            name,
            width,
            mask: writable_mask,
        });
    }
    if !fits_width(reset_value, width) {
        return Err(RegisterValidationError::ResetValueOverflow {
            name,
            width,
            value: reset_value,
        });
    }
    Ok(())
}

pub(super) fn validate_definition<I: Copy + Eq + fmt::Debug>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    validate_borrowed(
        definition,
        definition.fields(),
        root,
        path,
        declaration_index,
    )
}

impl<I: Copy + Eq + fmt::Debug> RegisterDefinition<I> {
    pub(crate) fn validate_fields(
        self,
        fields: &[RegisterFieldSpec<I>],
    ) -> Result<(), RegisterDefinitionError<I>> {
        validate_borrowed(self, fields, self.name(), self.name(), 0)
    }
}

fn validate_borrowed<I: Copy + Eq + fmt::Debug>(
    definition: RegisterDefinition<I>,
    fields: &[RegisterFieldSpec<I>],
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    validate_shape(definition, root, path, declaration_index)?;
    fields::validate(definition, fields, root, path, declaration_index)
}

pub(super) fn validate_shape<I: Copy>(
    definition: RegisterDefinition<I>,
    root: &'static str,
    path: &str,
    declaration_index: usize,
) -> Result<(), RegisterDefinitionError<I>> {
    if !supported_width(definition.width()) {
        return Err(RegisterDefinitionError::InvalidWidth {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            width: definition.width(),
        });
    }
    if definition.alignment() == 0 || !definition.alignment().is_power_of_two() {
        return Err(RegisterDefinitionError::InvalidAlignment {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            alignment: definition.alignment(),
        });
    }
    if !fits_width(definition.writable_mask(), definition.width()) {
        return Err(RegisterDefinitionError::WritableMaskOverflow {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            width: definition.width(),
            writable_mask: definition.writable_mask(),
        });
    }
    if !fits_width(definition.reset_value(), definition.width()) {
        return Err(RegisterDefinitionError::ResetValueOverflow {
            root,
            path: path.into(),
            declaration_index,
            name: definition.name(),
            width: definition.width(),
            reset_value: definition.reset_value(),
        });
    }
    Ok(())
}

pub(super) const fn fits_width(value: u64, width: usize) -> bool {
    width == 8 || (width < 8 && value < (1_u64 << (width * 8)))
}

const fn supported_width(width: usize) -> bool {
    matches!(width, 1 | 2 | 4 | 8)
}
