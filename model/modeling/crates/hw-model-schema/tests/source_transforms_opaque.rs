//! Inline opaque payloads retain their own prepared storage geometry.
use hw_model_schema::{
    Endian, FieldSpec, RecordPlacement, RecordSpec, ScalarKind,
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::{
        Definition, DefinitionBody, DefinitionId, DescribeSchema, Field, FieldType, Reference,
        ReferenceKind, SchemaSet, record::Node,
    },
    transform::{Inline, Outline},
    traversal::record,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const OPAQUE: RecordSpec = RecordSpec::named(
    "Opaque",
    2,
    2,
    &[FieldSpec::new(0, "value", 0, ScalarKind::U16)],
    &[],
);
const CONTAINER: RecordSpec = RecordSpec::named(
    "Container",
    6,
    2,
    &[],
    &[RecordPlacement::new(0, "child", 4, &OPAQUE)],
);
struct Opaque;
impl DescribeSchema for Opaque {
    fn schema_definition() -> Definition {
        Definition::opaque::<Self>(OPAQUE)
    }
}
struct Container;
impl DescribeSchema for Container {
    fn schema_definition() -> Definition {
        Definition::record::<Self>(
            CONTAINER,
            vec![Field::new(
                "child",
                4,
                2,
                Endian::Big,
                FieldType::Reference(Reference::new(
                    "child",
                    ReferenceKind::Field,
                    Opaque::schema_definition,
                )),
            )],
        )
    }
}
#[test]
fn inline_opaque_layout_is_local_and_reoutline_preserves_its_extent() -> TestResult {
    // Given opaque storage embedded at a nonzero outer offset.
    let mut set = SchemaSet::new();
    set.add::<Container>();
    // When explicitly expanded and analyzed.
    let graph = Inline::new().run(set)?.resolve()?;
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    // Then prepared opaque members remain local, with the source field owning its offset/endian.
    let mut found = 0;
    for root in record::roots(&schema) {
        check(root.context(), &mut found)?;
    }
    assert_eq!(found, 1);
    let outlined = Outline::new().run(graph)?.resolve()?;
    let root = outlined
        .get(DefinitionId::of::<Container>())
        .ok_or("root")?;
    assert!(
        matches!(root.references().first().ok_or("missing entry")?.target().body(), DefinitionBody::Opaque(spec) if spec.size() == 2)
    );
    AnalyzedSchema::from_graph(&outlined, &Bindings::new())?;
    Ok(())
}

fn check(ctx: &record::Context<'_>, found: &mut usize) -> TestResult {
    if let Node::Value(hw_model_schema::ast::value::Node::Opaque(_)) = ctx.node() {
        let layout = ctx.opaque_layout().ok_or("prepared local opaque")?;
        assert_eq!(layout.size(), 2);
        assert_eq!(layout.members().len(), 1);
        assert_eq!(
            layout
                .members()
                .first()
                .ok_or("missing entry")?
                .absolute_offset(),
            0
        );
        assert_eq!(ctx.field().ok_or("field")?.offset(), 4);
        assert_eq!(ctx.field().ok_or("field")?.endian(), Endian::Big);
        *found += 1;
    }
    for child in ctx.children() {
        check(child.context(), found)?;
    }
    Ok(())
}
