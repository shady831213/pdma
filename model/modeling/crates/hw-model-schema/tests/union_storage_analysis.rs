//! Storage projections are prepared at the union use site, not reconstructed by emitters.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{
        record,
        value::{self, Emit},
        view,
    },
    record_schema, traversal,
};
record_schema! { pub record Packet { endian: Little, fields: {
    kind: enum(u32) { variants: { Empty=0, Data=1 } },
    payload: tagged_union { tag:self.kind, cases:{ Empty {}, Data { number:u64 } } },
} } }
record_schema! { pub record Pair { endian:Little, fields:{ first:Packet, second:Packet } } }

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn check_prepared(ctx: &traversal::record::Context<'_>, count: &mut usize) -> TestResult {
    if let Some(reference) = ctx.node().reference()
        && reference.selector_binding().is_some()
    {
        let fields = ctx
            .analysis()
            .union_storage(ctx.id())
            .ok_or("prepared union use site")?;
        let start = ctx.field().ok_or("union field")?.offset();
        assert!(!fields.is_empty());
        assert!(fields.iter().all(|(_, f)| f.offset() >= start));
        *count += 1;
    }
    for child in ctx.children() {
        check_prepared(child.context(), count)?;
    }
    Ok(())
}
#[derive(Default)]
struct Probe(Vec<(String, usize)>);
impl value::Backend for Probe {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
}
impl record::Backend for Probe {
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        target: hw_model_schema::export::Visit<record::Context<'a>>,
        _: record::Context<'a>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        self.emit(target, w)
    }
    fn union(
        &mut self,
        _: view::Union<'_>,
        ctx: record::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        self.0.push((ctx.path(), ctx.union_storage().len()));
        self.emit_all(ctx.union_storage(), w)
    }
}
#[test]
fn union_projection_is_prepared_once_and_reused_at_both_reference_occurrences() -> TestResult {
    let mut source = SchemaSet::new();
    source.add::<Pair>();
    let schema = AnalyzedSchema::analyze(source, &Bindings::new()).unwrap();
    let mut count = 0;
    for root in traversal::record::definitions(&schema) {
        check_prepared(root.context(), &mut count)?;
    }
    assert_eq!(count, 1);
    let mut probe = Probe::default();
    probe
        .emit_all(
            record::entries(&schema).filter(|v| v.name() == Some("Pair")),
            &mut Vec::new(),
        )
        .unwrap();
    let [first, second] = probe.0.as_slice() else {
        return Err("expected both union occurrences".into());
    };
    assert_eq!(first.0, "first.payload");
    assert_eq!(second.0, "second.payload");
    assert!(first.1 > 0);
    assert_eq!(first.1, second.1);
    Ok(())
}
