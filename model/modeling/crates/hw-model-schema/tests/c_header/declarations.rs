use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::{
    ast::{DefinitionId, SchemaSet},
    bits_schema,
    export::backends::c_header::{CHeaderExportError, CRecordBackend},
    export::{ExportError, InMemoryArtifactSink},
};

bits_schema! { pub bits Flags(u8) { ready: 0..=0, } }

#[test]
fn record_macro_declarations_reject_collisions_after_the_significant_prefix()
-> super::support::TestResult {
    // Given: a valid record with one long caller-owned C prefix.
    let prefix = "a".repeat(64);
    let mut sink = InMemoryArtifactSink::new();

    // When: distinct metadata macros differ only after the C11 significant prefix.
    let schema = AnalyzedSchema::opaque("record", super::support::RECORD)?;
    let id = schema.view("record").ok_or("missing view")?.id.clone();
    let failure = hw_model_schema::analysis::Pipeline::new((&schema, ()))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?
        .generate(
            |data| CRecordBackend::new(data).with_name(id, &prefix),
            &mut sink,
        );

    // Then: the exact first pair is diagnosed before emitting an artifact.
    let stem = format!("C_{}", prefix.to_ascii_uppercase());
    assert_eq!(
        failure,
        Err(ExportError::Render(CHeaderExportError::NameCollision {
            first_path: format!("{stem}_SIZE"),
            second_path: format!("{stem}_ALIGNMENT"),
            first_index: 0,
            second_index: 1,
        }))
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}

#[test]
fn bits_macro_declarations_reject_collisions_after_the_significant_prefix()
-> super::support::TestResult {
    // Given: a selected bit definition with one long C name override.
    let mut schemas = SchemaSet::new();
    schemas.add::<Flags>();
    let graph = schemas.resolve()?;
    let prefix = "a".repeat(64);
    let mut sink = InMemoryArtifactSink::new();

    // When: generated size and bit-width macros alias in their significant prefix.
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let id = schema
        .definition(DefinitionId::of::<Flags>())
        .ok_or("missing flags")?
        .id
        .clone();
    let failure = hw_model_schema::analysis::Pipeline::new((&schema, ()))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?
        .generate(
            |data| CRecordBackend::new(data).with_name(id, &prefix),
            &mut sink,
        );

    // Then: the same typed declaration collision is returned without partial output.
    let stem = format!("C_{}", prefix.to_ascii_uppercase());
    assert_eq!(
        failure,
        Err(ExportError::Render(CHeaderExportError::NameCollision {
            first_path: format!("{stem}_SIZE"),
            second_path: format!("{stem}_BIT_WIDTH"),
            first_index: 0,
            second_index: 1,
        }))
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}

#[test]
fn accessor_function_names_use_the_final_c_symbol_scope() -> super::support::TestResult {
    use hw_model_schema::{FieldSpec, RecordSpec, ScalarKind};
    const PREFIX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const FIELDS: [FieldSpec; 2] = [
        FieldSpec::new(
            0,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab",
            0,
            ScalarKind::U8,
        ),
        FieldSpec::new(
            1,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaac",
            1,
            ScalarKind::U8,
        ),
    ];
    let schema = AnalyzedSchema::opaque("r", RecordSpec::new(2, 1, &FIELDS))?;
    let mut sink = InMemoryArtifactSink::new();
    let result = hw_model_schema::analysis::Pipeline::new((&schema, ()))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?
        .generate(CRecordBackend::new, &mut sink);
    let first = format!("C_R_LOAD_C_{}B", PREFIX.to_ascii_uppercase());
    let second = format!("C_R_LOAD_C_{}C", PREFIX.to_ascii_uppercase());
    assert_eq!(first.len(), 64);
    assert_eq!(
        result,
        Err(ExportError::Render(CHeaderExportError::NameCollision {
            first_path: first,
            second_path: second,
            first_index: 7,
            second_index: 12,
        }))
    );
    assert!(sink.artifacts().is_empty());
    Ok(())
}
