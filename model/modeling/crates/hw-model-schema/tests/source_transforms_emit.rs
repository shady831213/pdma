//! Public emission sees exactly the source containment chosen by the application.
use hw_model_schema::export::value::Emit as _;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass},
    ast::SchemaSet,
    export::{Visit, record, value, view},
    record_schema,
    transform::{Inline, Outline},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Leaf { endian: Big, fields: { value: u32 } } }
record_schema! { pub record Pair { endian: Little, fields: { left: Leaf, right: Leaf } } }
struct Probe;
impl record::Backend for Probe {
    fn reference<'a>(
        &mut self,
        reference: view::Reference<'a>,
        _: Visit<record::Context<'a>>,
        _: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        writeln!(writer, "reference:{}", reference.name())
    }
}
impl value::Backend for Probe {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
    fn scalar(
        &mut self,
        _: view::Scalar,
        ctx: record::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        writeln!(writer, "scalar:{}", ctx.path())
    }
}
fn emit(schema: &AnalyzedSchema) -> Result<String, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    Probe.emit_all(
        record::roots(schema).filter(|visit| visit.name() == Some("Pair")),
        &mut bytes,
    )?;
    Ok(String::from_utf8(bytes)?)
}
#[test]
fn source_transform_is_optional_and_emission_does_not_normalize() -> TestResult {
    // Given repeated explicit references in the source tree.
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let graph = set.resolve()?;
    let source = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    assert_eq!(emit(&source)?, "reference:left\nreference:right\n");
    // When the application selects inlining then invokes the same public emitter.
    let inline = Inline::new().run(&graph)?.resolve()?;
    let analyzed = AnalyzedSchema::from_graph(&inline, &Bindings::new())?;
    // Then two independent contained scalar occurrences are emitted.
    assert_eq!(emit(&analyzed)?, "scalar:left.value\nscalar:right.value\n");
    Ok(())
}
#[test]
fn outlined_roundtrip_is_reanalyzed_and_emitted_as_references() -> TestResult {
    // Given fully contained records after explicit expansion.
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    let inline = Inline::new().run(set)?.resolve()?;
    // When outlining and passing the result through the normal public analysis.
    let analyzed = AnalyzedSchema::analyze(Outline::new().run(&inline)?, &Bindings::new())?;
    // Then the emitter sees the requested named edges with no implicit expansion.
    assert_eq!(emit(&analyzed)?, "reference:left\nreference:right\n");
    Ok(())
}
record_schema! { pub record Tagged { endian: Little, fields: {
    kind: enum(u32) { variants: { Data = 7 } },
    payload: tagged_union { tag: self.kind, cases: { Data { number: u32 } } },
} } }
struct SelectorProbe;
impl record::Backend for SelectorProbe {
    fn reference<'a>(
        &mut self,
        reference: view::Reference<'a>,
        _: Visit<record::Context<'a>>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(
            ctx.field()
                .ok_or_else(|| std::io::Error::other("lexical field"))?
                .name(),
            "payload"
        );
        writeln!(
            writer,
            "{}:{}",
            ctx.path(),
            reference
                .selector_binding()
                .ok_or_else(|| std::io::Error::other("binding"))?
        )
    }
}
impl value::Backend for SelectorProbe {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
}
#[test]
fn renderer_observes_lifted_selector_at_its_lexical_field() -> TestResult {
    // Given an external tag that belongs to the source record, not the reusable payload.
    let mut set = SchemaSet::new();
    set.add::<Tagged>();
    // When the application outlines the union and invokes public emission.
    let schema = AnalyzedSchema::analyze(
        Outline::new()
            .select(|site| site.path() == ["payload"])
            .run(set)?,
        &Bindings::new(),
    )?;
    let mut bytes = Vec::new();
    SelectorProbe.emit_all(
        record::roots(&schema).filter(|visit| visit.name() == Some("Tagged")),
        &mut bytes,
    )?;
    // Then the reference callback retains the lexical field and its external selector binding.
    assert_eq!(String::from_utf8(bytes)?, "payload:self.kind\n");
    Ok(())
}
