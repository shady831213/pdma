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
    fn record_size(
        &mut self,
        size: usize,
        _: hw_model_schema::traversal::record::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.push(format!("size:record:{size}"), writer)?;
        Ok(())
    }
    fn field_size(
        &mut self,
        size: usize,
        ctx: hw_model_schema::traversal::record::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.push(
            format!(
                "size:{}:{size}",
                ctx.field().ok_or("missing field owner")?.name()
            ),
            writer,
        )?;
        Ok(())
    }
}
impl value::Backend for Recorder {
    type Domain = record::Domain;
    type Error = &'static str;
    type Context<'a> = hw_model_schema::traversal::record::Context<'a>;
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
fn documentation_and_geometry_reach_leaf_callbacks() -> Result<(), Box<dyn std::error::Error>> {
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
