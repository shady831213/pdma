use crate::output_support::MemoryOutput;
use hw_model_schema::analysis::{AnalyzedSchema, Bindings};
use hw_model_schema::{
    ast::{DefinitionId, SchemaSet},
    bits_schema,
    export::ExportError,
    export::backends::c_header::{CHeaderExportError, CRecordBackend},
};

bits_schema! { pub bits Flags(u8) { ready: 0..=0, } }

#[test]
fn record_macro_declarations_reject_collisions_after_the_significant_prefix()
-> super::support::TestResult {
    // Given: a valid record with one long caller-owned C prefix.
    let prefix = "a".repeat(64);
    let mut sink = MemoryOutput::new();

    // When: distinct metadata macros differ only after the C11 significant prefix.
    let schema = AnalyzedSchema::opaque("record", super::support::RECORD)?;
    let id = schema.view("record").ok_or("missing view")?.id.clone();
    let failure = sink.generate_prepared(
        &hw_model_schema::analysis::Pipeline::new((&schema, ()))
            .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?,
        |data| CRecordBackend::new(data).with_name(id, &prefix),
    );

    // Then: the exact first pair is diagnosed before emitting an artifact.
    let stem = format!("C_{}", prefix.to_ascii_uppercase());
    let ExportError::Render(CHeaderExportError::NameCollision {
        first_path,
        second_path,
        first_index,
        second_index,
    }) = failure.unwrap_err()
    else {
        panic!("expected name collision");
    };
    assert_eq!(first_path, format!("{stem}_SIZE"));
    assert_eq!(second_path, format!("{stem}_ALIGNMENT"));
    assert_eq!(first_index, 0);
    assert_eq!(second_index, 1);
    assert!(sink.outputs().is_empty());
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
    let mut sink = MemoryOutput::new();

    // When: generated size and bit-width macros alias in their significant prefix.
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let id = schema
        .definition(DefinitionId::of::<Flags>())
        .ok_or("missing flags")?
        .id
        .clone();
    let failure = sink.generate_prepared(
        &hw_model_schema::analysis::Pipeline::new((&schema, ()))
            .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?,
        |data| CRecordBackend::new(data).with_name(id, &prefix),
    );

    // Then: the same typed declaration collision is returned without partial output.
    let stem = format!("C_{}", prefix.to_ascii_uppercase());
    let ExportError::Render(CHeaderExportError::NameCollision {
        first_path,
        second_path,
        first_index,
        second_index,
    }) = failure.unwrap_err()
    else {
        panic!("expected name collision");
    };
    assert_eq!(first_path, format!("{stem}_SIZE"));
    assert_eq!(second_path, format!("{stem}_BIT_WIDTH"));
    assert_eq!(first_index, 0);
    assert_eq!(second_index, 1);
    assert!(sink.outputs().is_empty());
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
    let mut sink = MemoryOutput::new();
    let result = sink.generate_prepared(
        &hw_model_schema::analysis::Pipeline::new((&schema, ()))
            .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?,
        CRecordBackend::new,
    );
    let first = format!("C_R_LOAD_C_{}B", PREFIX.to_ascii_uppercase());
    let second = format!("C_R_LOAD_C_{}C", PREFIX.to_ascii_uppercase());
    assert_eq!(first.len(), 64);
    let ExportError::Render(CHeaderExportError::NameCollision {
        first_path,
        second_path,
        first_index,
        second_index,
    }) = result.unwrap_err()
    else {
        panic!("expected name collision");
    };
    assert_eq!(first_path, first);
    assert_eq!(second_path, second);
    assert_eq!(first_index, 7);
    assert_eq!(second_index, 12);
    assert!(sink.outputs().is_empty());
    Ok(())
}
