//! References may be named or expanded independently at each occurrence.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{DefinitionId, Origin, SchemaSet},
    export::{
        self, Visit, record,
        value::{self, Emit},
        view,
    },
    record_schema,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
record_schema! { pub record Leaf { endian: Big, fields: { value: u32 } } }
record_schema! { pub record Pair { endian: Little, fields: { left: Leaf, right: Leaf } } }
struct Probe {
    expand: bool,
    mixed: bool,
}
impl record::Backend for Probe {
    fn entry(
        &mut self,
        ctx: record::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(w, "entry:{}", ctx.entry_name())?;
        self.emit(ctx.current(), w)
    }
    fn reference<'a>(
        &mut self,
        reference: view::Reference<'a>,
        target: Visit<record::Context<'a>>,
        _: record::Context<'a>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        if self.expand && (!self.mixed || reference.name() == "left") {
            self.emit(target, w)
        } else {
            writeln!(w, "reference:{}", reference.name())
        }
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
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        assert_eq!(ctx.definition_origin(), Origin::Named);
        assert_eq!(
            ctx.definition_id().source_path(),
            Some(DefinitionId::of::<Leaf>().path())
        );
        writeln!(w, "scalar:{}:{}", ctx.entry_name(), ctx.path())
    }
}

fn render(
    expand: bool,
    mixed: bool,
    select_leaf: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Pair>();
    if select_leaf {
        set.add::<Leaf>();
    }
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    let mut backend = Probe { expand, mixed };
    for output in export::prepare_entries(
        &mut backend,
        record::entries(&schema).filter(|v| select_leaf || v.name() == Some("Pair")),
    )? {
        output.emit(&mut backend, &mut bytes)?;
    }
    Ok(String::from_utf8(bytes)?)
}
#[test]
fn named_reference_is_a_backend_choice() -> TestResult {
    assert_eq!(
        render(false, false, false)?,
        "entry:Pair\nreference:left\nreference:right\n"
    );
    Ok(())
}
#[test]
fn repeated_expansion_preserves_each_occurrence_and_one_output_entry() -> TestResult {
    assert_eq!(
        render(true, false, false)?,
        "entry:Pair\nscalar:Pair:left.value\nscalar:Pair:right.value\n"
    );
    Ok(())
}
#[test]
fn expansion_and_named_use_mix_without_rewriting_source() -> TestResult {
    assert_eq!(
        render(true, true, false)?,
        "entry:Pair\nscalar:Pair:left.value\nreference:right\n"
    );
    Ok(())
}
#[test]
fn referenced_definition_is_still_an_explicit_entry() -> TestResult {
    assert_eq!(
        render(true, false, true)?,
        "entry:Leaf\nscalar:Leaf:value\nentry:Pair\nscalar:Pair:left.value\nscalar:Pair:right.value\n"
    );
    Ok(())
}
