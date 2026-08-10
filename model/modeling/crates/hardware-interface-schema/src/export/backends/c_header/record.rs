use std::fmt::Write as _;

use crate::{
    Endian, RecordSpec, ScalarKind,
    export::{Artifact, ArtifactSink, Exporter},
};

use super::{
    C_MEDIA_TYPE, FragmentConfig,
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

impl Exporter<RecordSpec> for RecordCHeaderExporter {
    type Error = core::convert::Infallible;

    fn export<Sink>(&self, schema: &RecordSpec, sink: &mut Sink) -> Result<(), Self::Error>
    where
        Sink: ArtifactSink<Error = Self::Error>,
    {
        let model = RecordCHeaderModel::from_spec(*schema);
        sink.emit(Artifact {
            logical_name: self.config.logical_name.clone(),
            media_type: String::from(C_MEDIA_TYPE),
            contents: render(&model, &self.config.symbol_prefix).into_bytes(),
        })
    }
}

fn render(model: &RecordCHeaderModel, prefix: &str) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "#define {prefix}_SIZE UINT64_C({})", model.size());
    let _ = writeln!(
        output,
        "#define {prefix}_ALIGNMENT UINT64_C({})",
        model.alignment()
    );
    let _ = writeln!(output, "#define {prefix}_ENDIAN_LITTLE UINT32_C(0)");
    let _ = writeln!(output, "#define {prefix}_ENDIAN_BIG UINT32_C(1)");
    let names = disambiguate_identifiers(
        model
            .fields()
            .iter()
            .map(|field| c_identifier(field.name()))
            .collect(),
    );
    for (field, name) in model.fields().iter().copied().zip(names) {
        output.push('\n');
        push_field(&mut output, prefix, &name, field);
    }
    output
}

fn push_field(output: &mut String, prefix: &str, field_name: &str, field: RecordCHeaderField) {
    let metadata = format!("{prefix}_{field_name}");
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

fn push_load(output: &mut String, names: FieldNames<'_>, field: RecordCHeaderField) {
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

fn push_store(output: &mut String, names: FieldNames<'_>, field: RecordCHeaderField) {
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

const fn shift(field: RecordCHeaderField, byte_index: usize) -> usize {
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
