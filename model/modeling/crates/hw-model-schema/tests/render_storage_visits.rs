//! Behavioral coverage for restricted opaque storage traversal.
use hw_model_schema::{
    EnumDefinition, EnumVariantSpec, FieldSpec, RecordPlacement, RecordSpec, ScalarKind,
    analysis::{AnalyzedSchema, Bindings},
    ast::{Definition, SchemaSet},
    export::{self, Kind, record, value, value::Emit as _, view},
    traversal::NodeId,
};

struct Opaque;
const ENUM: EnumDefinition = EnumDefinition::new(&[EnumVariantSpec::new("Ready", 1)]);
const CHILD: RecordSpec = RecordSpec::named(
    "Child",
    4,
    4,
    &[FieldSpec::new(0, "state", 0, ScalarKind::U32).with_enum(ENUM)],
    &[],
);
const ROOT: RecordSpec = RecordSpec::named(
    "Root",
    12,
    4,
    &[FieldSpec::new(0, "head", 0, ScalarKind::U32)],
    &[RecordPlacement::repeated(1, "children", 4, 2, 4, &CHILD)],
);

#[derive(Default)]
struct Target {
    members: Vec<(String, NodeId)>,
    variants: Vec<String>,
    scalars: Vec<(String, ScalarKind, usize)>,
    enumerations: Vec<(String, ScalarKind, usize)>,
}
impl record::Backend for Target {}
impl value::Backend for Target {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
    fn scalar(
        &mut self,
        scalar: view::Scalar,
        ctx: Self::Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let path = ctx
            .storage_path()
            .ok_or_else(|| std::io::Error::other("missing scalar storage path"))?;
        self.scalars.push((path, scalar.scalar(), scalar.width()));
        Ok(())
    }
    fn enumeration(
        &mut self,
        enumeration: view::Enumeration,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let view::EnumEncoding::Scalar(scalar) = enumeration.encoding() else {
            return Err(std::io::Error::other("expected scalar enum encoding"));
        };
        let path = ctx
            .storage_path()
            .ok_or_else(|| std::io::Error::other("missing enum storage path"))?;
        self.enumerations
            .push((path, scalar.scalar(), scalar.width()));
        self.emit_all(ctx.children(), writer)
    }

    fn opaque(
        &mut self,
        _: view::Storage,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let fields: Vec<_> = ctx
            .children()
            .filter(|visit| visit.kind() == Kind::StorageMember)
            .collect();
        self.emit_all(fields.iter().rev().cloned(), writer)?;
        self.emit_all(fields, writer)
    }
    fn storage_member<'a>(
        &mut self,
        _: view::StorageMember<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let path = ctx
            .storage_path()
            .ok_or_else(|| std::io::Error::other("missing storage path"))?;
        self.members.push((path, ctx.id().clone()));
        self.emit_all(ctx.children(), writer)
    }
    fn variant(
        &mut self,
        value: &EnumVariantSpec,
        ctx: Self::Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let path = ctx
            .storage_path()
            .ok_or_else(|| std::io::Error::other("missing variant storage path"))?;
        self.variants.push(format!("{path}:{}", value.name()));
        Ok(())
    }
}

#[test]
fn opaque_members_keep_identity_and_nested_enum_context_when_reordered_and_repeated() {
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<Opaque>(ROOT));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut target = Target::default();
    for output in export::prepare(&schema, &mut target).unwrap() {
        output.emit(&mut target, &mut Vec::new()).unwrap();
    }
    let paths: Vec<_> = target
        .members
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "children[1].state",
            "children[1]",
            "children[0].state",
            "children[0]",
            "head",
            "head",
            "children[0]",
            "children[0].state",
            "children[1]",
            "children[1].state",
        ]
    );
    for (first, repeated) in target
        .members
        .iter()
        .take(5)
        .zip(target.members.iter().rev().take(5))
    {
        assert_eq!(first.1, repeated.1);
    }
    assert_eq!(
        target.variants,
        [
            "children[1].state:Ready",
            "children[0].state:Ready",
            "children[0].state:Ready",
            "children[1].state:Ready",
        ]
    );
    assert_eq!(
        target.scalars,
        [
            ("head".into(), ScalarKind::U32, 4),
            ("head".into(), ScalarKind::U32, 4)
        ]
    );
    assert_eq!(
        target.enumerations,
        [
            "children[1].state",
            "children[0].state",
            "children[0].state",
            "children[1].state"
        ]
        .map(|path| (path.into(), ScalarKind::U32, 4))
    );
}
