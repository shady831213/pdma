use std::fmt::Write as _;

use super::super::{
    COMPLETION, DESCRIPTOR, EVENT, FieldSpec, INDIRECT_DESTINATION_ENTRY, RecordSpec,
};

struct CRecord {
    macro_name: &'static str,
    size_name: &'static str,
    type_name: &'static str,
    spec: RecordSpec,
}

const RECORDS: [CRecord; 4] = [
    CRecord {
        macro_name: "DESCRIPTOR_V1",
        size_name: "PDMA_MODEL_ABI_DESCRIPTOR_SIZE",
        type_name: "pdma_model_abi_descriptor_v1",
        spec: DESCRIPTOR,
    },
    CRecord {
        macro_name: "COMPLETION_V1",
        size_name: "PDMA_MODEL_ABI_COMPLETION_SIZE",
        type_name: "pdma_model_abi_completion_v1",
        spec: COMPLETION,
    },
    CRecord {
        macro_name: "EVENT_V1",
        size_name: "PDMA_MODEL_ABI_EVENT_SIZE",
        type_name: "pdma_model_abi_event_v1",
        spec: EVENT,
    },
    CRecord {
        macro_name: "INDIRECT_DESTINATION_ENTRY_V1",
        size_name: "PDMA_MODEL_ABI_INDIRECT_DESTINATION_ENTRY_SIZE",
        type_name: "pdma_model_abi_indirect_destination_entry_v1",
        spec: INDIRECT_DESTINATION_ENTRY,
    },
];

pub(super) fn push(output: &mut String) {
    for record in &RECORDS {
        push_record(output, record);
    }
}

fn push_record(output: &mut String, record: &CRecord) {
    let prefix = format!("PDMA_MODEL_ABI_{}", record.macro_name);
    for field in record.spec.fields() {
        push_field_constants(output, &prefix, *field);
    }
    output.push('\n');
    let _ = writeln!(output, "typedef struct {} {{", record.type_name);
    for field in record.spec.fields() {
        let field_macro = field_macro(&prefix, *field);
        let _ = writeln!(output, "    uint8_t {}[{field_macro}_SIZE];", field.name());
    }
    let _ = writeln!(output, "}} {};", record.type_name);
    let _ = writeln!(
        output,
        "_Static_assert(_Alignof({}) == 1, \"{} byte alignment\");",
        record.type_name, record.type_name
    );
    let _ = writeln!(
        output,
        "_Static_assert(sizeof({}) == {}, \"{} size\");",
        record.type_name, record.size_name, record.type_name
    );
    for field in record.spec.fields() {
        push_field_assertions(output, record.type_name, &prefix, *field);
    }
    output.push('\n');
}

fn push_field_constants(output: &mut String, prefix: &str, field: FieldSpec) {
    let name = field_macro(prefix, field);
    let _ = writeln!(output, "#define {name}_OFFSET UINT16_C({})", field.offset());
    let _ = writeln!(output, "#define {name}_SIZE UINT16_C({})", field.width());
}

fn push_field_assertions(output: &mut String, type_name: &str, prefix: &str, field: FieldSpec) {
    let field_macro = field_macro(prefix, field);
    let _ = writeln!(
        output,
        "_Static_assert(offsetof({type_name}, {}) == {field_macro}_OFFSET, \"{} offset\");",
        field.name(),
        field.name()
    );
    let _ = writeln!(
        output,
        "_Static_assert(sizeof((({type_name} *)0)->{}) == {field_macro}_SIZE, \"{} width\");",
        field.name(),
        field.name()
    );
}

fn field_macro(prefix: &str, field: FieldSpec) -> String {
    format!("{prefix}_{}", field.name().to_ascii_uppercase())
}
