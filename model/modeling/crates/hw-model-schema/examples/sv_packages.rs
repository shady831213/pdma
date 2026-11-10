//! Emit record fragments and independent register layout packages to a caller-owned writer.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{
        backends::system_verilog::{SvRecordBackend, SvRegisterBackend},
        generate, prepare,
    },
    record_schema, register_map, register_schema,
};
use std::io::Write;
record_schema! {pub record Message {endian:Little,fields:{sequence:u32,payload:u64}}}
register_schema! {pub register Control {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:1,fields:{enabled:0..=0}}}
register_map! {pub map Peripheral(count:usize) {alignment:16,placements:[pub control:Control[count]]}}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut records = SchemaSet::new();
    records.add::<Message>();
    let records = AnalyzedSchema::analyze(records, &Bindings::new())?;
    let mut backend = SvRecordBackend::new();
    // Preparation checks entry names before the caller writes a package preamble.
    let entries = prepare(&records, &mut backend)?;
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    out.write_all(b"package message_records;\n")?;
    for entry in entries {
        entry.emit(&mut backend, &mut out)?;
    }
    out.write_all(b"endpackage\n")?;
    // Register packages retain symbolic count parameters; no hardware storage is created.
    let mut registers = SchemaSet::new();
    registers.add::<Peripheral>();
    let registers = AnalyzedSchema::analyze(registers, &Bindings::new())?;
    generate(&registers, SvRegisterBackend::new(), &mut out)?;
    out.flush()?;
    Ok(())
}
