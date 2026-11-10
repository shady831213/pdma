//! Emit RAL classes; the caller supplies the UVM package envelope and runtime integration.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{backends::uvm::UvmRegisterBackend, prepare},
    register_map, register_schema,
};
use std::io::Write;
register_schema! {pub register Status {width:4,endian:Little,access:ReadOnly,reset:Zero(0),writable_mask:0,fields:{ready:0..=0}}}
register_map! {pub map Peripheral(count:usize) {alignment:16,placements:[pub status:Status[count]]}}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Peripheral>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    // Defaults describe volatile registers and disable mirror comparisons. No predictor,
    // bus adapter or hardware side effect is inferred by the Rust renderer.
    let mut backend = UvmRegisterBackend::new();
    let entries = prepare(&schema, &mut backend)?;
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    out.write_all(b"package peripheral_ral;\n import uvm_pkg::*;\n `include \"uvm_macros.svh\"\n")?;
    for entry in entries {
        entry.emit(&mut backend, &mut out)?;
    }
    out.write_all(b"endpackage\n")?;
    out.flush()?;
    Ok(())
    // The SV caller creates Peripheral, calls build(count, bus_bytes), configures its
    // map/base, locks the model, and attaches the chosen adapter and sequencer.
}
