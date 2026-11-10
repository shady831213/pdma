//! Shared leaf dispatch preserves documentation text and typed ownership.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{record, value},
    record_schema,
};
use output_support::MemoryOutput;
record_schema! { #[short_description = "summary"] #[description = "first\nsecond"] pub record Documented { endian: Little, fields: { word: u32 } } }
#[derive(Default)]
struct Recorder(Vec<String>);
impl Recorder {
    fn push(&mut self, event: String, writer: &mut dyn std::io::Write) -> Result<(), &'static str> {
        if !self.0.is_empty() {
            writer.write_all(b"|").map_err(|_| "writer rejected")?;
        }
        writer
            .write_all(event.as_bytes())
            .map_err(|_| "writer rejected")?;
        self.0.push(event);
        Ok(())
    }
}
impl record::Backend for Recorder {
    fn record<'a>(
        &mut self,
        record: hw_model_schema::export::view::Record<'a>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        use hw_model_schema::export::value::Emit as _;
        self.push(format!("size:record:{}", record.spec().size()), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn field<'a>(
        &mut self,
        field: hw_model_schema::export::view::Field<'a>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        use hw_model_schema::export::value::Emit as _;
        assert_eq!(
            ctx.field().ok_or("missing field owner")?.name(),
            field.name()
        );
        self.push(format!("size:{}:{}", field.name(), field.width()), writer)?;
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for Recorder {
    type Domain = record::Domain;
    type Error = &'static str;
    type Context<'a> = hw_model_schema::export::record::Context<'a>;
    fn short_description(
        &mut self,
        text: &str,
        owner: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.push(format!("short:{}:{text}", owner.name), writer)?;
        Ok(())
    }
    fn description(
        &mut self,
        text: &str,
        owner: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.push(format!("full:{}:{text}", owner.name), writer)?;
        Ok(())
    }
}

#[test]
fn documentation_and_geometry_reach_owner_callbacks() -> Result<(), Box<dyn std::error::Error>> {
    // Given a documented record with one scalar field.
    let mut set = SchemaSet::new();
    set.add_record::<Documented>();
    let schema = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = MemoryOutput::new();
    // When common traversal invokes typed leaf handlers.
    sink.generate(&schema, Recorder::default())
        .map_err(|e| e.to_string())?;
    // Then whole documentation text precedes owner-specific sizes.
    assert_eq!(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
        b"short:Documented:summary|full:Documented:first\nsecond|size:record:4|size:word:4"
    );
    Ok(())
}
