use crate::{Endian, read_scalar, write_scalar};

use super::validation::fits_width;
use super::{RegisterDefinition, RegisterFieldSpec, RegisterValueError};

/// Checked immutable view of one whole-register value.
#[derive(Debug)]
pub struct RegisterValueView<'a, I: 'static = u16> {
    definition: RegisterDefinition<I>,
    bytes: &'a [u8],
}

impl<'a, I: Copy> RegisterValueView<'a, I> {
    pub(super) fn new(
        definition: RegisterDefinition<I>,
        bytes: &'a [u8],
    ) -> Result<Self, RegisterValueError> {
        if !view_shape_valid(definition) {
            return Err(RegisterValueError::InvalidDefinition);
        }
        let actual = bytes.len();
        let bytes =
            bytes
                .get(..definition.width())
                .ok_or_else(|| RegisterValueError::BufferTooShort {
                    required: definition.width(),
                    actual,
                })?;
        Ok(Self { definition, bytes })
    }

    /// Reads the complete value using the definition's byte order.
    pub fn value(&self) -> Result<u64, RegisterValueError> {
        decode(
            self.bytes,
            self.definition.width(),
            self.definition.endian(),
        )
    }

    /// Reads one field selected by its sibling name.
    pub fn field_by_name(&self, name: &'static str) -> Result<u64, RegisterValueError> {
        let field = find_field(self.definition, name)?;
        Ok(extract(self.value()?, field))
    }
}

/// Checked mutable view of one whole-register value.
#[derive(Debug)]
pub struct RegisterValueViewMut<'a, I: 'static = u16> {
    definition: RegisterDefinition<I>,
    bytes: &'a mut [u8],
}

impl<'a, I: Copy> RegisterValueViewMut<'a, I> {
    pub(super) fn new(
        definition: RegisterDefinition<I>,
        bytes: &'a mut [u8],
    ) -> Result<Self, RegisterValueError> {
        if !view_shape_valid(definition) {
            return Err(RegisterValueError::InvalidDefinition);
        }
        let actual = bytes.len();
        let bytes = bytes.get_mut(..definition.width()).ok_or_else(|| {
            RegisterValueError::BufferTooShort {
                required: definition.width(),
                actual,
            }
        })?;
        Ok(Self { definition, bytes })
    }

    /// Reads the complete current value.
    pub fn value(&self) -> Result<u64, RegisterValueError> {
        decode(
            self.bytes,
            self.definition.width(),
            self.definition.endian(),
        )
    }

    /// Replaces the complete value after checking its declared width.
    pub fn set_value(&mut self, value: u64) -> Result<(), RegisterValueError> {
        if !fits_width(value, self.definition.width()) {
            return Err(RegisterValueError::ValueOverflow {
                width: self.definition.width(),
                value,
            });
        }
        encode(
            self.bytes,
            self.definition.width(),
            self.definition.endian(),
            value,
        )
    }

    /// Reads one field selected by its sibling name.
    pub fn field_by_name(&self, name: &'static str) -> Result<u64, RegisterValueError> {
        let field = find_field(self.definition, name)?;
        Ok(extract(self.value()?, field))
    }

    /// Replaces one named field after checking the field width.
    pub fn set_field_by_name(
        &mut self,
        name: &'static str,
        value: u64,
    ) -> Result<(), RegisterValueError> {
        let field = find_field(self.definition, name)?;
        if !fits_bits(value, field.bit_width()) {
            return Err(RegisterValueError::FieldValueOverflow {
                field_name: field.name(),
                bit_width: field.bit_width(),
                value,
            });
        }
        let current = self.value()?;
        let field_mask = mask(field.bit_width()) << field.lsb();
        self.set_value((current & !field_mask) | (value << field.lsb()))
    }
}

fn find_field<I: Copy>(
    definition: RegisterDefinition<I>,
    name: &'static str,
) -> Result<RegisterFieldSpec<I>, RegisterValueError> {
    definition
        .fields()
        .iter()
        .copied()
        .find(|field| field.name() == name)
        .ok_or(RegisterValueError::UnknownFieldName { name })
}

const fn extract<I: Copy>(value: u64, field: RegisterFieldSpec<I>) -> u64 {
    (value >> field.lsb()) & mask(field.bit_width())
}

const fn mask(bits: usize) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

const fn fits_bits(value: u64, bits: usize) -> bool {
    bits >= 64 || value < (1_u64 << bits)
}

fn view_shape_valid<I: Copy>(definition: RegisterDefinition<I>) -> bool {
    matches!(definition.width(), 1 | 2 | 4 | 8)
        && definition.fields().iter().copied().all(|field| {
            field.bit_width() > 0
                && field
                    .range()
                    .is_some_and(|range| range.end <= definition.width() * 8)
        })
}

fn decode(bytes: &[u8], width: usize, endian: Endian) -> Result<u64, RegisterValueError> {
    let decoded = match width {
        1 => read_scalar::<u8>(bytes, endian).map(u64::from),
        2 => read_scalar::<u16>(bytes, endian).map(u64::from),
        4 => read_scalar::<u32>(bytes, endian).map(u64::from),
        8 => read_scalar::<u64>(bytes, endian),
        _ => return Err(RegisterValueError::ValueOverflow { width, value: 0 }),
    };
    decoded.map_err(codec_error)
}

fn encode(
    bytes: &mut [u8],
    width: usize,
    endian: Endian,
    value: u64,
) -> Result<(), RegisterValueError> {
    match width {
        1 => write_scalar(
            bytes,
            endian,
            u8::try_from(value).map_err(|_| overflow(width, value))?,
        ),
        2 => write_scalar(
            bytes,
            endian,
            u16::try_from(value).map_err(|_| overflow(width, value))?,
        ),
        4 => write_scalar(
            bytes,
            endian,
            u32::try_from(value).map_err(|_| overflow(width, value))?,
        ),
        8 => write_scalar(bytes, endian, value),
        _ => return Err(overflow(width, value)),
    }
    .map_err(codec_error)
}

const fn overflow(width: usize, value: u64) -> RegisterValueError {
    RegisterValueError::ValueOverflow { width, value }
}

const fn codec_error(error: crate::SchemaCodecError) -> RegisterValueError {
    match error {
        crate::SchemaCodecError::BufferTooShort { required, actual } => {
            RegisterValueError::BufferTooShort { required, actual }
        }
        crate::SchemaCodecError::UnknownTag { .. }
        | crate::SchemaCodecError::NonZeroReserved { .. }
        | crate::SchemaCodecError::NarrowingOverflow { .. }
        | crate::SchemaCodecError::ConstantMismatch { .. } => RegisterValueError::InvalidDefinition,
    }
}
