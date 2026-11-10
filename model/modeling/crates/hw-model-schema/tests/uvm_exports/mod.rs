//! RAL generation needs no device, runtime or simulator dependency in Rust.
#![cfg(feature = "export-uvm-regs")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{backends::uvm::UvmRegisterBackend, generate},
    register_map, register_schema,
};
register_schema! {pub register Word {width:4,endian:Little,access:ReadWrite,reset:Constant(2),writable_mask:3,fields:{mode:0..=1}}}
register_map! {pub map Root(n:usize) {alignment:8,placements:[pub words:Word[n]]}}
#[test]
fn standalone_uvm_preserves_fields_banks_and_full_width_parameters()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    generate(&schema, UvmRegisterBackend::new(), &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("class Root extends uvm_pkg::uvm_reg_block;"));
    assert!(text.contains("class Word extends uvm_pkg::uvm_reg;"));
    assert!(text.contains("input longint unsigned p_n"));
    assert!(text.contains("mode_field.configure(this, 2, 0, \"RW\", 1, 64'h2, 1, 0, 0)"));
    Ok(())
}

fn render(source: SchemaSet) -> Result<String, Box<dyn std::error::Error>> {
    let schema = AnalyzedSchema::analyze(source, &Bindings::new())?;
    let mut bytes = Vec::new();
    generate(&schema, UvmRegisterBackend::new(), &mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}
use hw_model_schema::export::{prepare_entries, register};
mod parameters;
mod policy;
mod source_bits;

mod aliases;

mod namespace;
mod native;
