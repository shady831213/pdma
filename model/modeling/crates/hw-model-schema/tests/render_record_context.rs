//! Sibling reordering keeps immutable field and bit ancestry attached to visits.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::export::record::Context as RecordContext;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{
        record::{self},
        value::{self, Emit as _},
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
        _: hw_model_schema::export::view::Record<'a>,
        ctx: RecordContext<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let mut fields: Vec<_> = ctx
            .children()
            .filter(|v| v.kind() == hw_model_schema::export::Kind::Field)
            .collect();
        fields.sort_by(|a, b| a.name().cmp(&b.name()));
        self.emit_all(fields, writer)
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
        assert_eq!(ctx.path(), field);
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
}

#[test]
fn sorted_sibling_visits_keep_original_field_and_bit_scopes() {
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
    for visit in <record::Domain as Domain<Probe>>::entries(&schema) {
        assert!(schema.units().any(|unit| &unit.id == visit.id()));
        assert!(visit.parent_id().is_none());
        probe.emit(visit, &mut std::io::sink()).unwrap();
    }
    assert_eq!(
        probe.0,
        ["alpha:second", "alpha:second", "zeta:first", "zeta:first"]
    );
}

struct RootNames(Vec<String>);
impl record::Backend for RootNames {
    fn reference<'a>(
        &mut self,
        _: hw_model_schema::export::view::Reference<'a>,
        target: hw_model_schema::export::Visit<RecordContext<'a>>,
        ctx: RecordContext<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let name = target.definition_name();
        if target.definition_origin() == hw_model_schema::ast::Origin::Named {
            assert!(self.0.iter().any(|root| root == name));
        } else {
            assert!(!self.0.iter().any(|root| root == name));
        }
        assert_eq!(target.parent_id().as_ref(), Some(ctx.id()));
        Ok(())
    }
    fn entry(
        &mut self,
        ctx: RecordContext<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert!(ctx.parent_id().is_none());
        {
            let event: String = ctx.entry_name().to_owned();
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
        _: hw_model_schema::export::view::Field<'a>,
        ctx: RecordContext<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.parent_id().as_ref(), Some(ctx.entry_id()));
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for RootNames {
    type Domain = record::Domain;
    type Error = &'static str;
    type Context<'a> = RecordContext<'a>;
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
