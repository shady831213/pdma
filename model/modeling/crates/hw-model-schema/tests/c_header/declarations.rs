use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::{
    ast::{DefinitionId, SchemaSet},
    bits_schema,
    export::backends::c_header::{CHeaderExportError, CRecordBackend},
    export::{ExportError, InMemoryArtifactSink, generate},
};

bits_schema! { pub bits Flags(u8) { ready: 0..=0, } }

#[test]
fn record_macro_declarations_reject_collisions_after_the_significant_prefix()
-> super::support::TestResult {
    // Given: a valid record with one long caller-owned C prefix.
    let prefix = "a".repeat(64);
    let renderer = CRecordBackend::new().with_name("record", &prefix);
    let mut sink = InMemoryArtifactSink::new();

    // When: distinct metadata macros differ only after the C11 significant prefix.
    let failure = generate(
        &AnalyzedSchema::opaque("record", super::support::RECORD)?,
        renderer,
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
    let renderer = CRecordBackend::new().with_name(DefinitionId::of::<Flags>(), &prefix);
    let mut sink = InMemoryArtifactSink::new();

    // When: generated size and bit-width macros alias in their significant prefix.
    let failure = generate(
        &AnalyzedSchema::from_graph(&graph, &Bindings::new())?,
        renderer,
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
