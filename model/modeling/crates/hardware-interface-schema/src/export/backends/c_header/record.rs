use std::fmt::Write as _;

use crate::{
    Endian, ResolvedRecord, ScalarKind,
    export::{Artifact, ArtifactSink, Exporter},
};

use super::{
    C_MEDIA_TYPE, CHeaderExportError, FragmentConfig,
    documentation::push_documentation,
    enumeration::push_enum,
    model::{RecordCHeaderField, RecordCHeaderModel},
    naming::{c_identifier, disambiguate_identifiers},
};

/// Renders one fixed record as a generic C11 fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCHeaderExporter {
    config: FragmentConfig,
}

impl RecordCHeaderExporter {
    /// Creates a renderer with caller-owned artifact and C symbol names.
    #[must_use]
    pub fn new(logical_name: impl Into<String>, symbol_prefix: impl AsRef<str>) -> Self {
        Self {
            config: FragmentConfig::new(logical_name, symbol_prefix),
        }
    }
}

impl Exporter<ResolvedRecord> for RecordCHeaderExporter {
    type Error = CHeaderExportError;

    fn export<Sink>(&self, schema: &ResolvedRecord, sink: &mut Sink) -> Result<(), Self::Error>
    where
        Sink: ArtifactSink<Error = Self::Error>,
    {
        let model = RecordCHeaderModel::from_resolved(schema);
        let paths_and_names = model
            .fields()
            .iter()
            .map(|field| (field.path().into(), c_identifier(field.name())))
            .collect::<Vec<_>>();
        super::naming::reject_collisions(&paths_and_names)?;
        sink.emit(Artifact {
            logical_name: self.config.logical_name.clone(),
            media_type: String::from(C_MEDIA_TYPE),
            contents: format!(
                "{}{}",
                render(&model, &self.config.symbol_prefix),
                render_placements(schema, &self.config.symbol_prefix)
            )
            .into_bytes(),
        })
    }
}

fn render_placements(schema: &ResolvedRecord, prefix: &str) -> String {
    let mut output = String::new();
    for member in schema.members() {
        if let crate::ResolvedMemberKind::Record { definition, .. } = member.kind() {
            let name = member
                .path()
                .strip_prefix(schema.root())
                .and_then(|path| path.strip_prefix('.'))
                .unwrap_or_else(|| member.path());
            let name = c_identifier(name);
            let _ = writeln!(
                output,
                "#define {prefix}_RECORD_{name}_OFFSET UINT64_C({})",
                member.absolute_offset()
            );
            let _ = writeln!(
                output,
                "#define {prefix}_RECORD_{name}_SIZE UINT64_C({})",
                definition.size()
            );
        }
    }
    output
}

fn render(model: &RecordCHeaderModel, prefix: &str) -> String {
    let mut output = String::new();
    push_documentation(&mut output, "", model.documentation());
    let _ = writeln!(output, "#define {prefix}_SIZE UINT64_C({})", model.size());
    let _ = writeln!(
        output,
        "#define {prefix}_ALIGNMENT UINT64_C({})",
        model.alignment()
    );
    let _ = writeln!(output, "#define {prefix}_ENDIAN_LITTLE UINT32_C(0)");
    let _ = writeln!(output, "#define {prefix}_ENDIAN_BIG UINT32_C(1)");
    for (path, documentation) in model.records() {
        push_documentation(&mut output, path, *documentation);
    }
    let names = disambiguate_identifiers(
        model
            .fields()
            .iter()
            .map(|field| c_identifier(field.name()))
            .collect(),
    );
    for (field, name) in model.fields().iter().zip(names) {
        output.push('\n');
        push_field(&mut output, prefix, &name, field);
    }
    output
}

fn push_field(output: &mut String, prefix: &str, field_name: &str, field: &RecordCHeaderField) {
    let metadata = format!("{prefix}_{field_name}");
    push_documentation(output, "", field.documentation());
    for bit in field.bits() {
        push_documentation(output, bit.name(), bit.documentation());
        if let Some(definition) = bit.definition().enum_definition() {
            push_enum(
                output,
                &format!("{metadata}_{}", c_identifier(bit.name())),
                definition,
            );
        }
    }
    if let Some(definition) = field.enum_definition() {
        push_enum(output, &metadata, definition);
    }
    let names = FieldNames {
        prefix,
        field: field_name,
        metadata: &metadata,
    };
    let _ = writeln!(
        output,
        "#define {metadata}_OFFSET UINT64_C({})",
        field.offset()
    );
    let _ = writeln!(
        output,
        "#define {metadata}_WIDTH UINT64_C({})",
        field.width()
    );
    let endian = match field.endian() {
        Endian::Little => "LITTLE",
        Endian::Big => "BIG",
    };
    let _ = writeln!(output, "#define {metadata}_ENDIAN {prefix}_ENDIAN_{endian}");
    push_load(output, names, field);
    push_store(output, names, field);
}

fn push_load(output: &mut String, names: FieldNames<'_>, field: &RecordCHeaderField) {
    let c_type = c_type(field.scalar());
    let FieldNames {
        prefix,
        field: field_name,
        metadata,
    } = names;
    let _ = writeln!(
        output,
        "static inline {c_type} {prefix}_LOAD_{field_name}(const uint8_t bytes[{prefix}_SIZE]) {{"
    );
    if field.width() == 1 {
        let _ = writeln!(output, "    return ({c_type})bytes[{metadata}_OFFSET];\n}}");
        return;
    }
    let _ = write!(output, "    return ({c_type})(");
    for byte_index in 0..field.width() {
        if byte_index > 0 {
            output.push_str(" | ");
        }
        let shift = shift(field, byte_index);
        if shift == 0 {
            let _ = write!(output, "({c_type})bytes[{metadata}_OFFSET");
        } else {
            let _ = write!(output, "(({c_type})bytes[{metadata}_OFFSET");
        }
        if byte_index > 0 {
            let _ = write!(output, " + UINT64_C({byte_index})");
        }
        if shift == 0 {
            output.push(']');
        } else {
            let _ = write!(output, "] << UINT32_C({shift}))");
        }
    }
    output.push_str(");\n}\n");
}

fn push_store(output: &mut String, names: FieldNames<'_>, field: &RecordCHeaderField) {
    let c_type = c_type(field.scalar());
    let FieldNames {
        prefix,
        field: field_name,
        metadata,
    } = names;
    let _ = writeln!(
        output,
        "static inline void {prefix}_STORE_{field_name}(uint8_t bytes[{prefix}_SIZE], {c_type} value) {{"
    );
    for byte_index in 0..field.width() {
        let _ = write!(output, "    bytes[{metadata}_OFFSET");
        if byte_index > 0 {
            let _ = write!(output, " + UINT64_C({byte_index})");
        }
        let shift = shift(field, byte_index);
        if shift == 0 {
            output.push_str("] = (uint8_t)(value & UINT64_C(255));\n");
        } else {
            let _ = writeln!(
                output,
                "] = (uint8_t)((value >> UINT32_C({shift})) & UINT64_C(255));"
            );
        }
    }
    output.push_str("}\n");
}

#[derive(Clone, Copy)]
struct FieldNames<'a> {
    prefix: &'a str,
    field: &'a str,
    metadata: &'a str,
}

const fn shift(field: &RecordCHeaderField, byte_index: usize) -> usize {
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
