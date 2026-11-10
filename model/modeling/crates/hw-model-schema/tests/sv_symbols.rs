//! Target identifiers are checked after naming, without restricting schema names.
#![cfg(feature = "export-sv")]
use hw_model_schema::{
    RecordSpec,
    analysis::{AnalyzedSchema, Bindings},
    ast::{Definition, DefinitionId, SchemaSet},
    export::{backends::system_verilog::SvRecordBackend, generate},
    record_schema,
};
record_schema! {pub record Foo {endian:Little,fields:{number:u32}}}
record_schema! {pub record FOO {endian:Little,fields:{number:u32}}}
record_schema! {pub record First {endian:Little,fields:{data_t:u32}}}
record_schema! {pub record Second {endian:Little,fields:{other:u32}}}
#[test]
fn case_folded_entry_conflict_is_preflighted_and_aliases_resolve_it()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Foo>();
    set.add::<FOO>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    assert!(generate(&schema, SvRecordBackend::new(), &mut bytes).is_err());
    assert!(bytes.is_empty());
    let id = schema
        .definition(DefinitionId::of::<FOO>())
        .ok_or("second")?
        .id
        .clone();
    generate(
        &schema,
        SvRecordBackend::new().with_name(id, "Other"),
        &mut bytes,
    )?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("Other_get_number"));
    assert!(text.contains("OTHER_SIZE"));
    Ok(())
}
#[test]
fn package_namespace_checks_functions_against_other_entry_types()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<First>();
    set.add::<Second>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let first = schema
        .definition(DefinitionId::of::<First>())
        .ok_or("first")?
        .id
        .clone();
    let second = schema
        .definition(DefinitionId::of::<Second>())
        .ok_or("second")?
        .id
        .clone();
    let backend = SvRecordBackend::new()
        .with_name(first, "A")
        .with_name(second, "A_get_data");
    let error = generate(&schema, backend, &mut Vec::new()).unwrap_err();
    assert!(error.to_string().contains("A_get_data_t"));
    Ok(())
}
#[test]
fn source_keyword_is_allowed_when_generated_symbols_are_not_keywords()
-> Result<(), Box<dyn std::error::Error>> {
    struct Keyword;
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<Keyword>(RecordSpec::named(
        "module",
        4,
        4,
        &[],
        &[],
    )));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    generate(&schema, SvRecordBackend::new(), &mut bytes)?;
    assert!(String::from_utf8(bytes)?.contains("module_t;"));
    Ok(())
}
