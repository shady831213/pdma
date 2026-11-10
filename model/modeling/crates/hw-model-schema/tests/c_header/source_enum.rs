//! An anonymous enum may also be explicitly selected as an independent output.
use crate::output_support::MemoryOutput;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pipeline},
    ast::{DefinitionId, SchemaSet},
    export::backends::c_header::{CRecordAnalysisPass, CRecordBackend},
    record_schema,
};
record_schema! { pub record Envelope { endian: Little, fields: {
    code: enum(u16) { variants: { Present = 0x1234 } },
} } }

#[test]
fn explicitly_selected_enum_emits_values_without_record_accessors() -> super::support::TestResult {
    let mut source = SchemaSet::new();
    source.add::<Envelope>();
    let graph = source.resolve()?;
    let envelope = graph
        .get(DefinitionId::of::<Envelope>())
        .ok_or("envelope")?;
    let definition = envelope.references().first().ok_or("code")?.target();
    let id = definition.id();
    let mut selected = SchemaSet::new();
    selected.add_definition(definition);
    let schema = AnalyzedSchema::analyze(selected, &Bindings::new())?;
    let id = schema.definition(id).ok_or("selected enum")?.id.clone();
    let prepared = Pipeline::new((&schema, ())).run(CRecordAnalysisPass)?;
    let mut output = MemoryOutput::new();
    output.generate_prepared(&prepared, |analysis| {
        CRecordBackend::new(analysis).with_name(id, "code")
    })?;
    assert_eq!(output.outputs().len(), 1);
    let text = &output.outputs().first().ok_or("enum output")?.bytes;
    super::support::compile_fragments(
        "standalone-enum",
        &[text.as_slice()],
        b"int main(void) { return C_CODE_C_QUPRESENT != 0x1234; }\n",
    )
}

hw_model_schema::register_schema! { pub register Status {
    width:4, endian:Little, access:ReadOnly, reset:Zero(0), writable_mask:0,
    fields:{ state:0..3 => enum { variants:{ Idle=0, Active=5 } } }
} }

#[test]
fn canonical_register_enum_reference_reaches_native_c() -> super::support::TestResult {
    use hw_model_schema::export::backends::c_header::CRegisterBackend;
    let mut source = SchemaSet::new();
    source.add::<Status>();
    let schema = AnalyzedSchema::analyze(source, &Bindings::new())?;
    let mut output = MemoryOutput::new();
    output.generate(&schema, CRegisterBackend::new())?;
    let bytes = &output.outputs().first().ok_or("register output")?.bytes;
    super::support::compile_fragments("source-register-enum",&[bytes.as_slice()],
        b"_Static_assert(C_STATUS_C_STATE_C_QUACTIVE == 5, \"register enum reference\");\nint main(void) { return 0; }\n")
}
