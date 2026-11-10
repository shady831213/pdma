//! Referenced records retain their own independent layout packages.
#![cfg(feature = "export-veryl-layout")]

use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{backends::veryl::VerylRecordBackend, generate},
    record_schema, union_schema,
};

record_schema! {
    #[short_description = "Small record"]
    pub record Small { endian: Little, fields: { value: u16 } }
}
record_schema! {
    #[short_description = "Large record"]
    pub record Large { endian: Little, fields: { value: u64 } }
}
union_schema! {
    #[short_description = "Complete command union"]
    pub union Command { cases: { Small(Small), Large(Large) } }
}

#[test]
fn root_union_references_do_not_reemit_record_package_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let mut independent = SchemaSet::new();
    independent.add::<Small>();
    independent.add::<Large>();
    let mut separate = Vec::new();
    generate(
        &AnalyzedSchema::analyze(independent, &Bindings::new())?,
        VerylRecordBackend::new(),
        &mut separate,
    )?;
    assert_eq!(
        std::str::from_utf8(&separate)?
            .matches("const EXTENT:")
            .count(),
        2
    );

    let mut selected = SchemaSet::new();
    selected.add::<Command>();
    let mut output = Vec::new();
    let result = generate(
        &AnalyzedSchema::analyze(selected, &Bindings::new())?,
        VerylRecordBackend::new(),
        &mut output,
    );
    assert!(
        result.is_ok(),
        "{result:?}\n{}",
        std::str::from_utf8(&output)?
    );
    let text = std::str::from_utf8(&output)?;
    let union = text
        .split("pub package CommandLayout {")
        .nth(1)
        .ok_or("missing union")?;
    assert_eq!(
        union,
        "\n    const EXTENT: u64 = 8;\n    const ALIGNMENT: u64 = 8;\n    // Complete command union\n}\n"
    );
    assert_eq!(text.matches("const EXTENT:").count(), 3);
    assert_eq!(text.matches("// Small record").count(), 1);
    assert_eq!(text.matches("// Large record").count(), 1);
    Ok(())
}
