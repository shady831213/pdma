//! Record and register generators can share a target namespace without sharing files.
#![cfg(feature = "export-veryl-layout")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{DefinitionId, SchemaSet},
    export::{
        EntrySymbols, ExportError,
        backends::veryl::{VerylExportError, VerylRecordBackend, VerylRegisterBackend},
        record, register,
    },
    record_schema, register_schema,
};
record_schema! {pub record Packet {endian:Little,fields:{data:u32}}}
register_schema! {pub register Word {width:4,endian:Little,access:ReadOnly,reset:Zero(0),writable_mask:0,fields:{}}}

#[test]
fn mixed_domains_reject_colliding_package_aliases_before_output()
-> Result<(), Box<dyn std::error::Error>> {
    // Given independent record/register backends generating one target namespace.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    set.add::<Word>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let id = |kind| {
        schema
            .definition(kind)
            .map(|v| v.id.clone())
            .ok_or("definition")
    };
    let mut record =
        VerylRecordBackend::new().with_name(id(DefinitionId::of::<Packet>())?, "Shared");
    let mut register =
        VerylRegisterBackend::new().with_name(id(DefinitionId::of::<Word>())?, "Shared");
    let mut symbols = EntrySymbols::new();
    let records = symbols.prepare(
        &mut record,
        record::entries(&schema).filter(|e| e.name() == Some("Packet")),
    )?;
    // When the second domain claims the same package, preparation fails before emit.
    let result = symbols.prepare(
        &mut register,
        register::entries(&schema).filter(|e| e.name() == Some("Word")),
    );
    assert!(
        matches!(result,Err(ExportError::Render(VerylExportError::Name(name))) if name=="Shared")
    );
    // Then routing remains source-owned even though target names are aliased.
    assert_eq!(records.first().ok_or("record")?.name(), "Packet");
    Ok(())
}
