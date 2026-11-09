use super::{AnalysisError, ShapeError};
use crate::ast::{BitsDefinition, CaseBody, DefinitionBody, Field, FieldType, UnionDefinition};
use crate::{Endian, EnumDefinition, FieldDefinition, RegisterDefinition};

pub(super) const fn failure(path: &'static str, source: ShapeError) -> AnalysisError {
    AnalysisError::SourceShape { path, source }
}
pub(super) fn field(field: &Field) -> Result<(), AnalysisError> {
    let path = field.name();
    let expected = match field.ty() {
        FieldType::Scalar {
            definition,
            constant,
        } => {
            scalar(path, *definition, field.endian())?;
            if constant.is_some_and(|value| !fits(value.value, definition.scalar().width() * 8)) {
                return Err(failure(path, ShapeError::ValueOverflow));
            }
            definition.scalar().width()
        }
        FieldType::Enum(definition) => {
            scalar(path, *definition, field.endian())?;
            if definition.enum_definition().is_none() {
                return Err(failure(path, ShapeError::MissingEnumeration));
            }
            definition.scalar().width()
        }
        FieldType::Bits(bits) => bits.scalar().width(),
        FieldType::Reference(reference) => extent(reference.target().body())
            .ok_or_else(|| failure(path, ShapeError::NonValueReference))?,
        FieldType::Record(record) => record.spec().size(),
        FieldType::Opaque(spec) => spec.size(),
        FieldType::Union(union) => union.storage().size(),
    };
    if field.width() != expected {
        return Err(failure(
            path,
            ShapeError::Width {
                declared: field.width(),
                expected,
            },
        ));
    }
    Ok(())
}
fn scalar(
    path: &'static str,
    definition: FieldDefinition,
    endian: Endian,
) -> Result<(), AnalysisError> {
    if definition.width() != definition.scalar().width() {
        return Err(failure(
            path,
            ShapeError::Width {
                declared: definition.width(),
                expected: definition.scalar().width(),
            },
        ));
    }
    if definition.endian() != endian {
        return Err(failure(path, ShapeError::Endian));
    }
    if let Some(enumeration) = definition.enum_definition() {
        enumeration_values(path, enumeration, definition.scalar().width() * 8)?;
    }
    bits(
        path,
        &BitsDefinition::new(definition.scalar(), definition.bits()),
    )
}
pub(super) fn bits(path: &'static str, bits: &BitsDefinition) -> Result<(), AnalysisError> {
    RegisterDefinition::new(path, bits.scalar().width(), Endian::Little)
        .with_fields(bits.fields())
        .validate()
        .map_err(|source| failure(path, ShapeError::Bits(source)))?;
    let derived = BitsDefinition::new(bits.scalar(), bits.fields());
    if derived.bit_width() != bits.bit_width() || derived.declared_mask() != bits.declared_mask() {
        return Err(failure(path, ShapeError::BitsMetadata));
    }
    for field in bits.fields() {
        if let Some(enumeration) = field.definition().enum_definition() {
            enumeration_values(field.name(), enumeration, field.bit_width())?;
        }
    }
    Ok(())
}
pub(super) fn enumeration_values(
    path: &'static str,
    enumeration: EnumDefinition,
    width: usize,
) -> Result<(), AnalysisError> {
    if enumeration.variants().is_empty() {
        return Err(failure(path, ShapeError::EmptyEnumeration));
    }
    for (index, variant) in enumeration.variants().iter().enumerate() {
        if !fits(variant.value(), width) {
            return Err(failure(path, ShapeError::ValueOverflow));
        }
        if enumeration
            .variants()
            .iter()
            .take(index)
            .any(|earlier| earlier.name() == variant.name() || earlier.value() == variant.value())
        {
            return Err(failure(path, ShapeError::DuplicateVariant));
        }
    }
    Ok(())
}
const fn fits(value: u64, width: usize) -> bool {
    width >= 64 || value < (1_u64 << width)
}
const fn extent(body: &DefinitionBody) -> Option<usize> {
    match body {
        DefinitionBody::Bits(bits) => Some(bits.scalar().width()),
        DefinitionBody::Record(record) => Some(record.spec().size()),
        DefinitionBody::Union(union) => Some(union.storage().size()),
        DefinitionBody::Opaque(spec) => Some(spec.size()),
        DefinitionBody::Register(_) | DefinitionBody::RegisterMap(_) => None,
    }
}
pub(super) fn union(union: &UnionDefinition) -> Result<(), AnalysisError> {
    union
        .storage()
        .resolve()
        .map_err(|source| AnalysisError::Record {
            path: union.storage().name(),
            source,
        })?;
    for case in union.cases() {
        let width = match case.body() {
            CaseBody::Bits(bits) => bits.scalar().width(),
            CaseBody::Union(union) => union.storage().size(),
            CaseBody::Opaque(spec) => spec.size(),
            CaseBody::Inline(record) => record.spec().size(),
            CaseBody::Reference(reference) => extent(reference.target().body())
                .ok_or_else(|| failure(case.name(), ShapeError::NonValueReference))?,
        };
        if width > union.storage().size() {
            return Err(failure(
                case.name(),
                ShapeError::Width {
                    declared: union.storage().size(),
                    expected: width,
                },
            ));
        }
    }
    Ok(())
}
