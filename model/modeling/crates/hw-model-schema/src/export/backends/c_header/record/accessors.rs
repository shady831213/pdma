use super::super::{CHeaderExportError, fragment::Fragment};
use crate::{Endian, FieldDefinition, ScalarKind};
use std::io::Write;

pub(super) fn scalar_accessors(
    output: &mut Fragment,
    names: FieldNames<'_>,
    field: FieldDefinition,
    writer: &mut dyn Write,
) -> Result<(), CHeaderExportError> {
    output.constants::<64>(names.metadata, &[("WIDTH", field.width())], writer)?;
    let endian = match field.endian() {
        Endian::Little => "LITTLE",
        Endian::Big => "BIG",
    };
    output.define(
        &format!("{}_ENDIAN", names.metadata),
        format_args!("{}_ENDIAN_{endian}", names.prefix),
        writer,
    )?;
    push_load(output, names, field, writer)?;
    push_store(output, names, field, writer)
}

fn push_load(
    output: &mut Fragment,
    names: FieldNames<'_>,
    field: FieldDefinition,
    writer: &mut dyn Write,
) -> Result<(), CHeaderExportError> {
    let c_type = c_type(field.scalar());
    let FieldNames {
        prefix,
        field: field_name,
        metadata,
    } = names;
    let function = format!("{prefix}_LOAD_{field_name}");
    output.declare(&function)?;
    let output = writer;
    writeln!(
        output,
        "static inline {c_type} {function}(const uint8_t bytes[{prefix}_SIZE]) {{"
    )?;
    if field.width() == 1 {
        writeln!(output, "    return ({c_type})bytes[{metadata}_OFFSET];\n}}")?;
        return Ok(());
    }
    write!(output, "    return ({c_type})(")?;
    for byte_index in 0..field.width() {
        if byte_index > 0 {
            output.write_all(b" | ")?;
        }
        let shift = shift(field, byte_index);
        if shift == 0 {
            write!(output, "({c_type})bytes[{metadata}_OFFSET")?;
        } else {
            write!(output, "(({c_type})bytes[{metadata}_OFFSET")?;
        }
        if byte_index > 0 {
            write!(output, " + UINT64_C({byte_index})")?;
        }
        if shift == 0 {
            output.write_all(b"]")?;
        } else {
            write!(output, "] << UINT32_C({shift}))")?;
        }
    }
    output.write_all(b");\n}\n")?;
    Ok(())
}

fn push_store(
    output: &mut Fragment,
    names: FieldNames<'_>,
    field: FieldDefinition,
    writer: &mut dyn Write,
) -> Result<(), CHeaderExportError> {
    let c_type = c_type(field.scalar());
    let FieldNames {
        prefix,
        field: field_name,
        metadata,
    } = names;
    let function = format!("{prefix}_STORE_{field_name}");
    output.declare(&function)?;
    let output = writer;
    writeln!(
        output,
        "static inline void {function}(uint8_t bytes[{prefix}_SIZE], {c_type} value) {{"
    )?;
    for byte_index in 0..field.width() {
        write!(output, "    bytes[{metadata}_OFFSET")?;
        if byte_index > 0 {
            write!(output, " + UINT64_C({byte_index})")?;
        }
        let shift = shift(field, byte_index);
        if shift == 0 {
            output.write_all(b"] = (uint8_t)(value & UINT64_C(255));\n")?;
        } else {
            writeln!(
                output,
                "] = (uint8_t)((value >> UINT32_C({shift})) & UINT64_C(255));"
            )?;
        }
    }
    output.write_all(b"}\n")?;
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct FieldNames<'a> {
    pub(super) prefix: &'a str,
    pub(super) field: &'a str,
    pub(super) metadata: &'a str,
}

const fn shift(field: FieldDefinition, byte_index: usize) -> usize {
    match field.endian() {
        Endian::Little => byte_index * 8,
        Endian::Big => (field.width() - byte_index - 1) * 8,
    }
}

const fn c_type(scalar: ScalarKind) -> &'static str {
    match scalar {
        ScalarKind::U8 => "uint8_t",
        ScalarKind::U16 => "uint16_t",
        ScalarKind::U32 => "uint32_t",
        ScalarKind::U64 => "uint64_t",
    }
}
