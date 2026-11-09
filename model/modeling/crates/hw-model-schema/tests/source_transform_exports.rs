//! Applications choose structural transforms before target-specific generation.
#![cfg(feature = "export-c-header")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::SchemaSet,
    export::{
        backends::c_header::{CRecordAnalysisPass, CRecordBackend},
        prepare_roots, record,
    },
    record_schema,
    transform::Inline,
};

record_schema! { pub record Leaf { endian:Big, fields:{value:u16} } }
record_schema! { pub record Pair { endian:Little, fields:{first:Leaf, gap:u32, second:Leaf} } }

fn render(inline: bool) -> Result<String, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let graph = set.resolve()?;
    let graph = if inline {
        Inline::new().run(&graph)?.resolve()?
    } else {
        graph
    };
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let (_, analysis) = CRecordAnalysisPass.run((&schema, ()))?;
    let mut backend = CRecordBackend::new(&analysis);
    let outputs = prepare_roots(
        &mut backend,
        record::roots(&schema).filter(|root| root.name() == Some("Pair")),
    )?;
    let mut bytes = Vec::new();
    for output in outputs {
        output.emit(&mut backend, &mut bytes)?;
    }
    Ok(String::from_utf8(bytes)?)
}

#[test]
fn c_renderer_does_not_implicitly_inline_referenced_records()
-> Result<(), Box<dyn std::error::Error>> {
    let text = render(false)?;
    assert!(text.contains("C_PAIR_RECORD_C_FIRST_OFFSET"));
    assert!(!text.contains("C_PAIR_LOAD_C_FIRSTQX2EVALUE"));
    Ok(())
}

#[test]
fn explicit_inlining_produces_occurrence_specific_accessors()
-> Result<(), Box<dyn std::error::Error>> {
    let text = render(true)?;
    assert!(text.contains("C_PAIR_C_FIRSTQX2EVALUE_OFFSET UINT64_C(0)"));
    assert!(text.contains("C_PAIR_C_SECONDQX2EVALUE_OFFSET UINT64_C(8)"));
    assert!(text.contains("C_PAIR_LOAD_C_FIRSTQX2EVALUE"));
    assert!(text.contains("C_PAIR_LOAD_C_SECONDQX2EVALUE"));
    Ok(())
}
