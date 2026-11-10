//! C chooses use-site expansion without changing the canonical graph.
#![cfg(feature = "export-c-header")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::SchemaSet,
    export::{
        backends::c_header::{CRecordAnalysisPass, CRecordBackend},
        prepare_entries, record,
    },
    record_schema,
};
record_schema! { pub record Leaf { endian:Big, fields:{value:u16} } }
record_schema! { pub record Pair { endian:Little, fields:{first:Leaf, gap:u32, second:Leaf} } }
#[test]
fn c_expansion_emits_distinct_accessors_without_dependency_output_wrappers()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let (_, analysis) = CRecordAnalysisPass.run((&schema, ()))?;
    let mut bytes = Vec::new();
    let mut backend = CRecordBackend::new(&analysis);
    for output in prepare_entries(
        &mut backend,
        record::entries(&schema).filter(|v| v.name() == Some("Pair")),
    )? {
        output.emit(&mut backend, &mut bytes)?;
    }
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("C_PAIR_C_FIRSTQX2EVALUE_OFFSET UINT64_C(0)"));
    assert!(text.contains("C_PAIR_C_SECONDQX2EVALUE_OFFSET UINT64_C(8)"));
    assert!(text.contains("C_PAIR_LOAD_C_FIRSTQX2EVALUE"));
    assert!(text.contains("C_PAIR_LOAD_C_SECONDQX2EVALUE"));
    assert!(!text.contains("#define C_LEAF_SIZE"));
    Ok(())
}
