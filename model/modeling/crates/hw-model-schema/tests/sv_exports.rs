//! Builtin SV exports are independent from application contracts.
#![cfg(feature = "export-sv")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{
        backends::system_verilog::{SvRecordBackend, SvRegisterBackend},
        generate,
    },
    record_schema, register_map, register_schema,
};
record_schema! {pub record Message {endian:Big,fields:{word:u32}}}
record_schema! {pub record Empty {endian:Little,fields:{}}}
record_schema! {pub record WithEmpty {endian:Little,fields:{empty:Empty,number:u8}}}
register_schema! {pub register Status {width:4,endian:Little,access:ReadOnly,reset:Constant(1),writable_mask:0,fields:{ready:0..=0}}}
register_map! {pub map Ports(n:usize) {alignment:16,placements:[pub status:Status[n]]}}
#[test]
fn sv_record_and_register_backends_consume_common_analysis()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Message>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    generate(&schema, SvRecordBackend::new(), &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("Message_t"));
    assert!(text.contains("Message_get_word"));
    let mut set = SchemaSet::new();
    set.add::<Ports>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    generate(&schema, SvRegisterBackend::new(), &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("package PortsLayout;"));
    assert!(text.contains("function automatic logic [63:0] status_count("));
    assert!(text.contains("input logic [63:0] p_n"));
    assert!(!text.contains("uvm_"));
    Ok(())
}
#[test]
fn zero_sized_record_is_rejected_before_output() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Empty>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    assert!(generate(&schema, SvRecordBackend::new(), &mut bytes).is_err());
    assert!(bytes.is_empty());
    Ok(())
}
#[test]
fn zero_sized_referenced_field_returns_error_instead_of_underflow()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<WithEmpty>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut backend = SvRecordBackend::new();
    let entries = hw_model_schema::export::prepare_entries(
        &mut backend,
        hw_model_schema::export::record::entries(&schema).filter(|v| v.name() == Some("WithEmpty")),
    )?;
    assert!(
        entries
            .first()
            .ok_or("entry")?
            .emit(&mut backend, &mut Vec::new())
            .is_err()
    );
    Ok(())
}
