use std::fmt::Write as _;

use hardware_interface_schema::export::backends::c_header::{
    RecordCHeaderField, RecordCHeaderModel,
};

struct CRecord {
    macro_stem: &'static str,
    size_constant: &'static str,
    c_type: &'static str,
}

const RECORDS: [CRecord; 4] = [
    CRecord {
        macro_stem: "DESCRIPTOR_V1",
        size_constant: "PDMA_MODEL_ABI_DESCRIPTOR_SIZE",
        c_type: "pdma_model_abi_descriptor_v1",
    },
    CRecord {
        macro_stem: "COMPLETION_V1",
        size_constant: "PDMA_MODEL_ABI_COMPLETION_SIZE",
        c_type: "pdma_model_abi_completion_v1",
    },
    CRecord {
        macro_stem: "EVENT_V1",
        size_constant: "PDMA_MODEL_ABI_EVENT_SIZE",
        c_type: "pdma_model_abi_event_v1",
    },
    CRecord {
        macro_stem: "INDIRECT_DESTINATION_ENTRY_V1",
        size_constant: "PDMA_MODEL_ABI_INDIRECT_DESTINATION_ENTRY_SIZE",
        c_type: "pdma_model_abi_indirect_destination_entry_v1",
    },
];

pub(super) fn push(output: &mut String, models: &[RecordCHeaderModel; 4]) {
    for (record, model) in RECORDS.iter().zip(models) {
        push_record(output, record, model);
    }
}

fn push_record(output: &mut String, record: &CRecord, model: &RecordCHeaderModel) {
    let prefix = format!("PDMA_MODEL_ABI_{}", record.macro_stem);
    for field in model.fields() {
        push_field_constants(output, &prefix, field);
    }
    output.push('\n');
    let _ = writeln!(output, "typedef struct {} {{", record.c_type);
    for field in model.fields() {
        let field_macro = field_macro(&prefix, field);
        let _ = writeln!(
            output,
            "    uint8_t {}[{field_macro}_SIZE];",
            leaf_name(field)
        );
    }
    let _ = writeln!(output, "}} {};", record.c_type);
    let _ = writeln!(
        output,
        "_Static_assert(_Alignof({}) == 1, \"{} byte alignment\");",
        record.c_type, record.c_type
    );
    let _ = writeln!(
        output,
        "_Static_assert(sizeof({}) == {}, \"{} size\");",
        record.c_type, record.size_constant, record.c_type
    );
    for field in model.fields() {
        push_field_assertions(
            output,
            FieldAssertion {
                type_name: record.c_type,
                prefix: &prefix,
                field,
            },
        );
    }
    output.push('\n');
}

fn push_field_constants(output: &mut String, prefix: &str, field: &RecordCHeaderField) {
    let name = field_macro(prefix, field);
    let _ = writeln!(output, "#define {name}_OFFSET UINT16_C({})", field.offset());
    let _ = writeln!(output, "#define {name}_SIZE UINT16_C({})", field.width());
}

fn push_field_assertions(output: &mut String, assertion: FieldAssertion<'_>) {
    let FieldAssertion {
        type_name,
        prefix,
        field,
    } = assertion;
    let field_macro = field_macro(prefix, field);
    let _ = writeln!(
        output,
        "_Static_assert(offsetof({type_name}, {}) == {field_macro}_OFFSET, \"{} offset\");",
        leaf_name(field),
        leaf_name(field)
    );
    let _ = writeln!(
        output,
        "_Static_assert(sizeof((({type_name} *)0)->{}) == {field_macro}_SIZE, \"{} width\");",
        leaf_name(field),
        leaf_name(field)
    );
}

fn field_macro(prefix: &str, field: &RecordCHeaderField) -> String {
    format!("{prefix}_{}", leaf_name(field).to_ascii_uppercase())
}

fn leaf_name(field: &RecordCHeaderField) -> &str {
    field
        .name()
        .rsplit_once('.')
        .map_or_else(|| field.name(), |(_, name)| name)
}

#[derive(Clone, Copy)]
struct FieldAssertion<'a> {
    type_name: &'a str,
    prefix: &'a str,
    field: &'a RecordCHeaderField,
}
