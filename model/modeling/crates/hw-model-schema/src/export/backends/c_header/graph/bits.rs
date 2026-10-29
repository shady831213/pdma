use super::super::{C_MEDIA_TYPE, enumeration::push_enum, naming::c_identifier};
use crate::{export::Artifact, graph::BitsDefinition};
use std::fmt::Write;

pub(super) fn render(name: &str, layout: &BitsDefinition) -> Result<Artifact, std::fmt::Error> {
    let prefix = c_identifier(name);
    let mut text = String::new();
    writeln!(
        text,
        "#define {prefix}_SIZE UINT64_C({})",
        layout.scalar().width()
    )?;
    writeln!(
        text,
        "#define {prefix}_BIT_WIDTH UINT32_C({})",
        layout.bit_width()
    )?;
    writeln!(
        text,
        "#define {prefix}_DECLARED_MASK UINT64_C({})",
        layout.declared_mask()
    )?;
    for field in layout.fields() {
        let member = format!("{prefix}_{}", c_identifier(field.name()));
        writeln!(text, "#define {member}_LSB UINT32_C({})", field.lsb())?;
        writeln!(
            text,
            "#define {member}_BITS UINT32_C({})",
            field.bit_width()
        )?;
        if let Some(enumeration) = field.definition().enum_definition() {
            push_enum(&mut text, &member, enumeration);
        }
    }
    Ok(Artifact {
        logical_name: name.into(),
        media_type: C_MEDIA_TYPE.into(),
        contents: text.into_bytes(),
    })
}
