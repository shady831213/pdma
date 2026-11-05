//! Target-language identifiers are checked before output is written.
#![cfg(feature = "export-veryl-layout")]
use hw_model_schema::{
    RecordSpec,
    analysis::AnalyzedSchema,
    export::{
        ExportError,
        backends::veryl::{VerylExportError, VerylRecordBackend},
        generate,
    },
};

#[test]
fn reserved_package_names_fail_before_emission() -> Result<(), Box<dyn std::error::Error>> {
    // Given: valid storage and aliases that are Veryl keyword tokens.
    let schema = AnalyzedSchema::opaque("Storage", RecordSpec::new(4, 4, &[]))?;
    let id = schema.view("Storage").ok_or("missing storage")?.id.clone();
    for name in [
        "wire", "_", "__hidden", "package", "module", "function", "input", "u32", "true", "clock",
        "unsafe",
    ] {
        let mut bytes = Vec::new();
        // When: the target checks its package namespace.
        let result = generate(
            &schema,
            VerylRecordBackend::new().with_name(id.clone(), name),
            &mut bytes,
        );
        // Then: reserved tokens are rejected without a partial package.
        assert!(
            matches!(result, Err(ExportError::Render(VerylExportError::Name(found))) if found == name)
        );
        assert!(bytes.is_empty());
    }
    Ok(())
}

#[test]
fn keyword_prefixes_and_different_case_are_identifiers() -> Result<(), Box<dyn std::error::Error>> {
    // Given: identifiers that contain a keyword without being that token.
    let schema = AnalyzedSchema::opaque("Storage", RecordSpec::new(4, 4, &[]))?;
    let id = schema.view("Storage").ok_or("missing storage")?.id.clone();
    for name in ["Package", "packageLayout", "input_data", "u32_value"] {
        let mut bytes = Vec::new();
        // When: each alias is rendered.
        generate(
            &schema,
            VerylRecordBackend::new().with_name(id.clone(), name),
            &mut bytes,
        )?;
        // Then: its spelling and layout remain intact.
        let text = String::from_utf8(bytes)?;
        assert!(text.contains(&format!("pub package {name} {{")));
        assert!(text.contains("const EXTENT: u64 = 4;"));
    }
    Ok(())
}
