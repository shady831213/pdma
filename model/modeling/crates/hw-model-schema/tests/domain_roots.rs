//! Backend-independent roots filter domains without weakening generation preflight.
/// Caller-owned capture for generation tests.
pub mod output_support;
use core::convert::Infallible;
use hw_model_schema::{
    RecordSpec,
    analysis::{AnalysisError, AnalyzedSchema, Bindings},
    ast::{Definition, SchemaSet},
    bits_schema,
    export::{ExportError, register, value},
    record_schema, register_map, register_schema, union_schema,
};
use output_support::MemoryOutput;
bits_schema! { pub bits Flags(u8) { enabled: 0..1 } }
record_schema! { pub record Payload { endian: Little, fields: { flags: bits(Flags) } } }
union_schema! { pub union Choice { cases: { Payload(Payload) } } }
register_schema! { pub register Word {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:0xffff_ffff,fields:{}} }
register_map! { pub map Block {alignment:4,placements:[pub word:Word]} }
struct Storage;
fn mixed() -> Result<AnalyzedSchema, AnalysisError> {
    let mut set = SchemaSet::new();
    set.add::<Block>();
    set.add::<Choice>();
    set.add_definition(Definition::opaque::<Storage>(RecordSpec::named(
        "Storage",
        4,
        1,
        &[],
        &[],
    )));
    AnalyzedSchema::analyze(set, &Bindings::new())
}
#[test]
fn mixed_schema_roots_keep_only_the_selected_domain_and_shared_values() {
    let schema = mixed().unwrap();
    let mut registers: Vec<_> = hw_model_schema::traversal::register::definitions(&schema)
        .map(|v| v.context().unit().name.as_str())
        .collect();
    let mut records: Vec<_> = hw_model_schema::traversal::record::definitions(&schema)
        .map(|v| v.context().unit().name.as_str())
        .collect();
    registers.sort_unstable();
    records.sort_unstable();
    assert_eq!(registers, ["Block", "Flags", "Storage", "Word"]);
    assert_eq!(records, ["Choice", "Flags", "Payload", "Storage"]);
}
struct Probe<'a>(&'a std::cell::Cell<usize>);
impl register::Backend for Probe<'_> {
    fn entry(
        &mut self,
        _: hw_model_schema::export::register::Context<'_>,
        _writer: &mut dyn std::io::Write,
    ) -> Result<(), Infallible> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
}
impl value::Backend for Probe<'_> {
    type Domain = register::Domain;
    type Error = Infallible;
    type Context<'a> = hw_model_schema::export::register::Context<'a>;
}

#[test]
fn generation_rejects_mixed_schema_before_filtered_root_callbacks() {
    let schema = mixed().unwrap();
    let callbacks = std::cell::Cell::new(0);
    let mut sink = MemoryOutput::new();
    let result = sink.generate(&schema, Probe(&callbacks));
    assert!(matches!(
        result,
        Err(ExportError::Analysis(AnalysisError::Domain {
            domain: "register",
            ..
        }))
    ));
    assert_eq!(callbacks.get(), 0);
    assert!(sink.outputs().is_empty());
}
