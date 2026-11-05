//! Cross-parent reordering keeps immutable field and bit ancestry attached to visits.
use hw_model_schema::traversal::record::Context as RecordContext;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{RecordDefinition, SchemaSet, record::Node},
    export::{
        Artifact, InMemoryArtifactSink, generate,
        record::{self},
        value::{self, Backend},
    },
    record_schema,
};
record_schema! { pub record Packet { endian: Little, fields: {
    zeta: bits(u8) { fields: { first: 0..2 => enum { variants: { Off = 0, On = 1 } } } },
    alpha: bits(u8) { fields: { second: 0..2 => enum { variants: { Off = 0, On = 1 } } } },
} } }
record_schema! { pub record Envelope { endian: Little, fields: { packet: Packet } } }
struct Probe(Vec<String>);
impl record::Backend for Probe {
    fn record<'a>(
        &mut self,
        _: &'a RecordDefinition,
        ctx: RecordContext<'a>,
    ) -> Result<(), Self::Error> {
        let mut grandchildren: Vec<_> = ctx
            .children()
            .filter(|v| matches!(v.node(), Node::Field(_)))
            .flat_map(|v| v.context().children())
            .collect();
        grandchildren.sort_by_key(|v| v.context().field().map(hw_model_schema::ast::Field::name));
        self.emit_all(grandchildren)
    }
}
impl value::Backend for Probe {
    type Domain = record::Domain;
    type Error = &'static str;
    type Context<'a> = RecordContext<'a>;
    fn variant(
        &mut self,
        _: &hw_model_schema::EnumVariantSpec,
        ctx: RecordContext<'_>,
    ) -> Result<(), Self::Error> {
        let field = ctx.field().ok_or("missing field")?.name();
        let bit = ctx.bit().ok_or("missing bit")?;
        assert!(
            ctx.ancestors()
                .iter()
                .any(|n| matches!(n, Node::Field(f) if f.name() == field))
        );
        self.0.push(format!("{field}:{bit}"));
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "parents".into(),
            media_type: "text/plain".into(),
            contents: self.0.join("\n").into_bytes(),
        }])
    }
}
#[test]
fn grandchildren_sorted_across_siblings_keep_original_parent() {
    let mut set = SchemaSet::new();
    set.add_record::<Packet>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut sink = InMemoryArtifactSink::new();
    generate(&schema, Probe(vec![]), &mut sink).unwrap();
    assert_eq!(
        sink.artifacts().first().unwrap().contents,
        b"alpha:second\nalpha:second\nzeta:first\nzeta:first"
    );
}

#[test]
fn root_visit_retains_owner_when_dispatched_as_an_ordinary_visit() {
    use hw_model_schema::export::Domain;
    let mut set = SchemaSet::new();
    set.add_record::<Packet>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut probe = Probe(vec![]);
    for visit in <record::Domain as Domain<Probe>>::roots(&schema) {
        assert!(core::ptr::eq(
            visit.context().analysis(),
            core::ptr::from_ref(&schema)
        ));
        assert!(visit.context().ancestors().is_empty());
        probe.emit(visit).unwrap();
    }
    assert_eq!(
        probe.0,
        ["alpha:second", "alpha:second", "zeta:first", "zeta:first"]
    );
}

struct RootNames(Vec<String>);
impl record::Backend for RootNames {
    fn root(&mut self, ctx: RecordContext<'_>) -> Result<(), Self::Error> {
        assert!(ctx.ancestors().is_empty());
        self.0.push(ctx.unit().name.clone());
        self.emit(ctx.current())
    }
    fn field<'a>(
        &mut self,
        _: &'a hw_model_schema::ast::Field,
        ctx: RecordContext<'a>,
    ) -> Result<(), Self::Error> {
        assert!(matches!(ctx.ancestors(), [Node::Record(_)]));
        self.emit_all(ctx.children())
    }
}
impl value::Backend for RootNames {
    type Domain = record::Domain;
    type Error = &'static str;
    type Context<'a> = RecordContext<'a>;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "roots".into(),
            media_type: "text/plain".into(),
            contents: self.0.join("\n").into_bytes(),
        }])
    }
}
#[test]
fn generation_dispatches_dependency_roots_without_extra_ancestors() {
    let mut set = SchemaSet::new();
    set.add_record::<Envelope>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut sink = InMemoryArtifactSink::new();
    generate(&schema, RootNames(vec![]), &mut sink).unwrap();
    assert_eq!(
        sink.artifacts().first().unwrap().contents,
        b"Packet\nEnvelope"
    );
}
