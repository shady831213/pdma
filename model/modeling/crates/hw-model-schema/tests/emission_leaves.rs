//! Shared leaf dispatch preserves documentation text and typed ownership.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{self, Artifact, InMemoryArtifactSink, record, value},
    record_schema,
};
record_schema! { #[short_description = "summary"] #[description = "first\nsecond"] pub record Documented { endian: Little, fields: { word: u32 } } }
#[derive(Default)]
struct Recorder(Vec<String>);
impl record::Backend for Recorder {
    fn record_size(&mut self, size: usize, _: record::EmitContext<'_>) -> Result<(), Self::Error> {
        self.0.push(format!("size:record:{size}"));
        Ok(())
    }
    fn field_size(&mut self, size: usize, ctx: record::EmitContext<'_>) -> Result<(), Self::Error> {
        self.0.push(format!(
            "size:{}:{size}",
            ctx.field().ok_or("missing field owner")?.name()
        ));
        Ok(())
    }
}
impl value::Backend for Recorder {
    type Domain = record::Domain;
    type Error = &'static str;
    type Context<'a> = record::EmitContext<'a>;
    fn short_description(
        &mut self,
        text: &str,
        owner: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("short:{}:{text}", owner.name));
        Ok(())
    }
    fn description(
        &mut self,
        text: &str,
        owner: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("full:{}:{text}", owner.name));
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "trace".into(),
            media_type: "text/plain".into(),
            contents: self.0.join("|").into_bytes(),
        }])
    }
}
#[test]
fn documentation_and_geometry_reach_leaf_callbacks() -> Result<(), Box<dyn std::error::Error>> {
    // Given a documented record with one scalar field.
    let mut set = SchemaSet::new();
    set.add_record::<Documented>();
    let schema = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = InMemoryArtifactSink::new();
    // When common traversal invokes typed leaf handlers.
    export::generate(&schema, Recorder::default(), &mut sink).map_err(|e| e.to_string())?;
    // Then whole documentation text precedes owner-specific sizes.
    assert_eq!(
        sink.artifacts().first().ok_or("missing artifact")?.contents,
        b"short:Documented:summary|full:Documented:first\nsecond|size:record:4|size:word:4"
    );
    Ok(())
}
