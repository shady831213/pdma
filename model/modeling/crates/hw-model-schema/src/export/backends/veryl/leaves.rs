use super::VerylExportError;
use crate::{
    Access, Endian,
    graph::{DefinitionBody, LayoutError, MapDefinition, RegisterInfo},
};
use std::collections::HashSet;
use std::fmt::Write;

pub(super) fn register(
    text: &mut String,
    prefix: &str,
    offset: usize,
    register: &RegisterInfo,
) -> Result<(), VerylExportError> {
    if !matches!(register.width(), 1 | 2 | 4 | 8) {
        return Err(VerylExportError::Register(prefix.into()));
    }
    let (read, write) = match register.access() {
        Access::ReadOnly => (true, false),
        Access::WriteOnly => (false, true),
        Access::ReadWrite => (true, true),
    };
    let little = register.endian() == Endian::Little;
    writeln!(text, "    const {prefix}_ENDIAN_LITTLE: bbool = {little};")?;
    if let Some(description) = register.documentation().short_description {
        for line in description.lines() {
            writeln!(text, "    // {line}")?;
        }
    }
    write!(
        text,
        "    const {prefix}_OFFSET: u64 = {offset};\n    const {prefix}_BYTES: u32 = {};\n    const {prefix}_RESET: u64 = 64'h{:016x};\n    const {prefix}_WRITABLE: u64 = 64'h{:016x};\n    const {prefix}_READ: bbool = {read};\n    const {prefix}_WRITE: bbool = {write};\n",
        register.width(),
        register.reset_value(),
        register.writable_mask()
    )?;
    for field in register.fields() {
        let name = field.name().to_ascii_uppercase();
        write!(
            text,
            "    const {prefix}_{name}_LSB: u32 = {};\n    const {prefix}_{name}_BITS: u32 = {};\n",
            field.lsb(),
            field.bit_width()
        )?;
        if let Some(enumeration) = field.definition().enum_definition() {
            for variant in enumeration.variants() {
                writeln!(
                    text,
                    "    const {prefix}_{name}_{}: u64 = {};",
                    variant.name().to_ascii_uppercase(),
                    variant.value()
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn map(
    text: &mut String,
    definition: &MapDefinition,
    arguments: &[usize],
    base: usize,
    prefix: &str,
    names: &mut HashSet<String>,
) -> Result<(), VerylExportError> {
    let geometry = definition.resolve(arguments)?;
    for (placement, resolved) in definition.placements().iter().zip(&geometry.placements) {
        let child = placement.reference().target();
        let name = if prefix.is_empty() {
            resolved.name.to_ascii_uppercase()
        } else {
            format!("{prefix}_{}", resolved.name.to_ascii_uppercase())
        };
        for index in 0..resolved.count {
            let name = if placement.is_bank() {
                format!("{name}_{index}")
            } else {
                name.clone()
            };
            let offset = index
                .checked_mul(resolved.stride)
                .and_then(|step| step.checked_add(resolved.offset))
                .and_then(|offset| offset.checked_add(base))
                .ok_or(LayoutError::Geometry)?;
            match child.body() {
                DefinitionBody::Register(register_info) => {
                    if !names.insert(name.clone()) {
                        return Err(VerylExportError::Name(name));
                    }
                    register(text, &name, offset, register_info)?;
                }
                DefinitionBody::RegisterMap(child_map) => {
                    map(text, child_map, &resolved.arguments, offset, &name, names)?;
                }
                DefinitionBody::Bits(_)
                | DefinitionBody::Record(_)
                | DefinitionBody::Union { .. }
                | DefinitionBody::Opaque(_) => return Err(LayoutError::Geometry.into()),
            }
        }
    }
    Ok(())
}
