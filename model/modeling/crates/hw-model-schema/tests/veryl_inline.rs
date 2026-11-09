//! Inlined union payloads retain their containing package boundary.
#![cfg(feature = "export-veryl-layout")]
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::{
        Case, Definition, DefinitionBody, DescribeSchema, Reference, ReferenceKind, SchemaSet,
        UnionDefinition,
    },
    bits_schema,
    export::{backends::veryl::VerylRecordBackend, generate},
    record_schema,
    transform::Inline,
    union_schema,
};

record_schema! { pub record Small { endian:Little, fields:{value:u16} } }
record_schema! { pub record Large { endian:Little, fields:{value:u64} } }
bits_schema! { pub bits Flags(u16) {ready:0..=0} }
union_schema! {
    #[short_description = "Choice envelope"]
    pub union Choice { cases:{Small(Small),Large(Large)} }
}

#[test]
fn inlined_payload_records_do_not_repeat_package_extent() -> Result<(), Box<dyn std::error::Error>>
{
    let mut set = SchemaSet::new();
    let definition = Choice::schema_definition();
    let DefinitionBody::Union(union) = definition.body() else {
        return Err("union".into());
    };
    let cases = union
        .cases()
        .iter()
        .cloned()
        .chain([Case::external(Reference::new(
            "Flags",
            ReferenceKind::Variant,
            Flags::schema_definition,
        ))])
        .collect();
    set.add_definition(Definition::from_body(
        definition.id(),
        definition.name().into(),
        definition.documentation(),
        DefinitionBody::Union(UnionDefinition::new(
            union.storage(),
            union.selector(),
            cases,
        )),
    ));
    let schema = AnalyzedSchema::analyze(Inline::new().run(set)?, &Bindings::new())?;
    let mut bytes = Vec::new();
    generate(&schema, VerylRecordBackend::new(), &mut bytes)?;
    let text = String::from_utf8(bytes)?;
    assert_eq!(text.matches("pub package ").count(), 1);
    assert_eq!(text.matches("const EXTENT:").count(), 1);
    assert_eq!(text.matches("Choice envelope").count(), 1);
    assert!(text.contains("const EXTENT: u64 = 8;"));
    Ok(())
}
