//! Cross-parent reordering keeps immutable field and bit ancestry attached to visits.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::traversal::record::Context as RecordContext;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{RecordDefinition, SchemaSet, record::Node},
    export::{
        record::{self},
        value::{self, Backend},
    },
    record_schema,
};
use output_support::MemoryOutput;
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
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let mut grandchildren: Vec<_> = ctx
            .children()
            .filter(|v| matches!(v.node(), Node::Field(_)))
            .flat_map(|v| v.context().children())
            .collect();
        grandchildren.sort_by_key(|v| v.context().field().map(hw_model_schema::ast::Field::name));
        self.emit_all(grandchildren, writer)
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
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let field = ctx.field().ok_or("missing field")?.name();
        let bit = ctx.bit().ok_or("missing bit")?;
        assert!(
            ctx.ancestors()
                .iter()
                .any(|n| matches!(n, Node::Field(f) if f.name() == field))
        );
        {
            let event: String = format!("{field}:{bit}");
            if !self.0.is_empty() {
                writer.write_all(b"\n").map_err(|_| "writer rejected")?;
            }
            writer
                .write_all(event.as_bytes())
                .map_err(|_| "writer rejected")?;
            self.0.push(event);
        }
        Ok(())
    }
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/plain".into(),
        })
    }
}
#[test]
fn grandchildren_sorted_across_siblings_keep_original_parent() {
    let mut set = SchemaSet::new();
    set.add_record::<Packet>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut sink = MemoryOutput::new();
    sink.generate(&schema, Probe(vec![])).unwrap();
    assert_eq!(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
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
        probe.emit(visit, &mut std::io::sink()).unwrap();
    }
    assert_eq!(
        probe.0,
        ["alpha:second", "alpha:second", "zeta:first", "zeta:first"]
    );
}

struct RootNames(Vec<String>);
impl record::Backend for RootNames {
    fn root(
        &mut self,
        ctx: RecordContext<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert!(ctx.ancestors().is_empty());
        {
            let event: String = ctx.unit().name.clone();
            if !self.0.is_empty() {
                writer.write_all(b"\n").map_err(|_| "writer rejected")?;
            }
            writer
                .write_all(event.as_bytes())
                .map_err(|_| "writer rejected")?;
            self.0.push(event);
        }
        self.emit(ctx.current(), writer)
    }
    fn field<'a>(
        &mut self,
        _: &'a hw_model_schema::ast::Field,
        ctx: RecordContext<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert!(matches!(ctx.ancestors(), [Node::Record(_)]));
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for RootNames {
    type Domain = record::Domain;
    type Error = &'static str;
    type Context<'a> = RecordContext<'a>;
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/plain".into(),
        })
    }
}
#[test]
fn generation_dispatches_dependency_roots_without_extra_ancestors() {
    let mut set = SchemaSet::new();
    set.add_record::<Envelope>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut sink = MemoryOutput::new();
    sink.generate(&schema, RootNames(vec![])).unwrap();
    assert_eq!(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
        b"Packet\nEnvelope"
    );
}
