//! C preparation retains distinct source occurrences.
#![cfg(feature = "export-c-header")]
pub mod output_support;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pipeline},
    ast::SchemaSet,
    export::backends::c_header::{CRecordAnalysisPass, CRecordBackend},
    record_schema,
};
use output_support::MemoryOutput;
record_schema! { pub record Leaf { endian: Little, fields: { value: u16 } } }
record_schema! { pub record Pair { endian: Little, fields: { first: Leaf, gap: u32, second: Leaf } } }
#[test]
fn prepared_occurrences_keep_reference_site_offsets() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add_record::<Pair>();
    let graph = set.resolve()?;
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let prepared = Pipeline::new((&schema, ())).run(CRecordAnalysisPass)?;
    let id = &schema
        .definition(hw_model_schema::ast::DefinitionId::of::<Pair>())
        .ok_or("pair definition")?
        .id;
    let root = hw_model_schema::traversal::record::roots(&schema)
        .find(|visit| visit.context().id() == id)
        .ok_or("pair root")?;
    let mut offsets = Vec::new();
    for child in root.context().children() {
        let hw_model_schema::ast::record::Node::Field(field) = child.node() else {
            continue;
        };
        if let Some(referent) = child.context().referent() {
            for leaf in referent.context().children() {
                if matches!(leaf.node(), hw_model_schema::ast::record::Node::Field(_)) {
                    offsets.push((field.name(), prepared.input().1.offset(leaf.context().id())));
                }
            }
        }
    }
    assert_eq!(offsets, [("first", Some(0)), ("second", Some(8))]);
    let mut sink = MemoryOutput::new();
    sink.generate_prepared(&prepared, CRecordBackend::new)?;
    let text = sink
        .outputs()
        .iter()
        .find(|a| a.metadata.logical_name == "pair")
        .ok_or("pair missing")?;
    let text = std::str::from_utf8(&text.bytes)?;
    assert!(text.contains("C_SECONDQX2EVALUE_OFFSET UINT64_C(8)"));
    assert!(text.contains("C_FIRSTQX2EVALUE_OFFSET UINT64_C(0)"));
    Ok(())
}

#[test]
fn duplicate_source_names_keep_distinct_offsets_and_fail_target_naming()
-> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::{
        Endian, FieldDefinition, RecordSpec, ScalarKind,
        ast::{Definition, Field, FieldType},
        export::backends::c_header::CHeaderExportError,
    };
    let field = |name, offset| {
        Field::new(
            name,
            offset,
            1,
            Endian::Little,
            FieldType::Scalar {
                definition: FieldDefinition::new(ScalarKind::U8),
                constant: None,
            },
        )
    };
    let mut set = SchemaSet::new();
    set.add_definition(Definition::record::<()>(
        RecordSpec::named("Duplicate", 3, 1, &[], &[]),
        vec![
            field("first", 0),
            field("duplicate", 1),
            field("duplicate", 2),
        ],
    ));
    let graph = set.resolve()?;
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let prepared = Pipeline::new((&schema, ())).run(CRecordAnalysisPass)?;
    let root = hw_model_schema::traversal::record::roots(&schema)
        .next()
        .ok_or("record root")?;
    let fields: Vec<_> = root
        .context()
        .children()
        .filter(|visit| matches!(visit.node(), hw_model_schema::ast::record::Node::Field(_)))
        .map(|visit| {
            (
                visit.context().id().clone(),
                prepared.input().1.offset(visit.context().id()),
            )
        })
        .collect();
    assert_eq!(
        fields.iter().map(|(_, offset)| *offset).collect::<Vec<_>>(),
        [Some(0), Some(1), Some(2)]
    );
    assert_ne!(
        fields.get(1).ok_or("second field")?.0,
        fields.get(2).ok_or("third field")?.0
    );
    let mut sink = MemoryOutput::new();
    let error = sink
        .generate_prepared(&prepared, CRecordBackend::new)
        .unwrap_err();
    let hw_model_schema::export::ExportError::Render(CHeaderExportError::NameCollision {
        first_path,
        second_path,
        first_index,
        second_index,
    }) = error
    else {
        panic!("expected name collision");
    };
    assert_eq!(first_path, "C_DUPLICATE_C_DUPLICATE_OFFSET".to_string());
    assert_eq!(second_path, "C_DUPLICATE_C_DUPLICATE_OFFSET".to_string());
    assert_eq!(first_index, 9);
    assert_eq!(second_index, 14);
    assert!(sink.outputs().is_empty());
    Ok(())
}
