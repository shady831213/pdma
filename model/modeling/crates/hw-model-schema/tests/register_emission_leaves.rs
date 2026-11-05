//! Register leaf callbacks receive effective fields and owner-specific geometry.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::export::value::Backend as _;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{LayoutExpr, SchemaSet},
    bits_schema,
    export::{self, register, value},
    register_map, register_schema,
};
use output_support::MemoryOutput;
bits_schema! {
    #[short_description = "flags"]
    pub bits Flags(u32) {
        #[short_description = "enabled"]
        #[description = "bit first\nbit second"]
        enabled: 0..1,
    }
}
register_schema! {
    #[short_description = "register"]
    #[description = "register detail"]
    pub register Control {width:4, endian:Little, access:ReadWrite, reset:Zero(0), writable_mask:1, fields:Flags,}
}
register_map! {
    #[description = "map detail"]
    pub map Device {alignment:4, placements:[
        #[short_description = "placement"]
        control: Control,
    ]}
}
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Description,
    Write(std::io::ErrorKind),
}
#[derive(Default)]
struct Trace {
    events: Vec<String>,
    fail: bool,
}
impl Trace {
    fn push(&mut self, event: String, writer: &mut dyn std::io::Write) -> Result<(), Failure> {
        if !self.events.is_empty() {
            writer
                .write_all(b"|")
                .map_err(|error| Failure::Write(error.kind()))?;
        }
        writer
            .write_all(event.as_bytes())
            .map_err(|error| Failure::Write(error.kind()))?;
        self.events.push(event);
        Ok(())
    }
}
impl register::Backend for Trace {
    fn register_width(
        &mut self,
        width: usize,
        ctx: hw_model_schema::traversal::register::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        assert!(
            ctx.register()
                .is_some_and(|register| register.source().is_some())
        );
        self.push(format!("width:{width}"), writer)?;
        Ok(())
    }
    fn register_alignment(
        &mut self,
        alignment: usize,
        _: hw_model_schema::traversal::register::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        self.push(format!("register-alignment:{alignment}"), writer)?;
        Ok(())
    }
    fn map_alignment(
        &mut self,
        alignment: usize,
        _: hw_model_schema::traversal::register::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        self.push(format!("map-alignment:{alignment}"), writer)?;
        Ok(())
    }
    fn register_field<'a>(
        &mut self,
        field: &'a hw_model_schema::BitsFieldSpec<usize>,
        ctx: hw_model_schema::traversal::register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        self.push(format!("field:{}", field.name()), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn layout_offset(
        &mut self,
        expression: &LayoutExpr,
        _: hw_model_schema::traversal::register::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        assert_eq!(*expression, LayoutExpr::Constant(0));
        self.push("offset:0".into(), writer)?;
        Ok(())
    }
    fn layout_extent(
        &mut self,
        expression: &LayoutExpr,
        _: hw_model_schema::traversal::register::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        assert_eq!(*expression, LayoutExpr::Constant(4));
        self.push("extent:4".into(), writer)?;
        Ok(())
    }
}
impl value::Backend for Trace {
    type Domain = register::Domain;
    type Error = Failure;
    type Context<'a> = hw_model_schema::traversal::register::Context<'a>;
    fn short_description(
        &mut self,
        text: &str,
        owner: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        self.push(
            format!("short:{:?}:{}:{text}", owner.kind, owner.name),
            writer,
        )?;
        Ok(())
    }
    fn description(
        &mut self,
        text: &str,
        owner: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        if self.fail {
            return Err(Failure::Description);
        }
        self.push(
            format!("full:{:?}:{}:{text}", owner.kind, owner.name),
            writer,
        )?;
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
fn schema() -> Result<AnalyzedSchema, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Control>();
    Ok(AnalyzedSchema::from_graph(
        &set.resolve()?,
        &Bindings::new(),
    )?)
}
#[test]
fn referenced_fields_are_visited_once_per_owner_with_both_doc_fragments()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a reusable bits definition embedded in a register.
    let schema = schema()?;
    let mut sink = MemoryOutput::new();
    // When default shared recursion emits both selected definitions.
    sink.generate(&schema, Trace::default())
        .expect("generation");
    // Then reference metadata remains present, while the register owns one effective field visit.
    let text = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    assert_eq!(text.matches("field:enabled").count(), 1);
    assert_eq!(text.matches("short:BitField:enabled:enabled").count(), 2);
    assert_eq!(
        text.matches("full:BitField:enabled:bit first\nbit second")
            .count(),
        2
    );
    assert!(text.contains("short:Register:Control:register|full:Register:Control:register detail|width:4|register-alignment:4"));
    Ok(())
}
#[test]
fn documentation_failure_is_typed_and_publishes_no_partial_artifact()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a backend rejecting full descriptions after an accepted summary.
    let schema = schema()?;
    let mut sink = MemoryOutput::new();
    // When a descendant documentation callback fails.
    let result = sink.generate(
        &schema,
        Trace {
            fail: true,
            ..Trace::default()
        },
    );
    // Then the exact error escapes and the caller discards the failed root.
    assert_eq!(
        result,
        Err(export::ExportError::Render(Failure::Description))
    );
    assert!(sink.outputs().is_empty());
    Ok(())
}
#[test]
fn map_and_register_geometry_dispatch_have_distinct_owners()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a resolved map with completed constant geometry.
    let mut set = SchemaSet::new();
    set.add::<Device>();
    let schema = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = MemoryOutput::new();
    // When shared traversal emits map formulas and concrete fields.
    sink.generate(&schema, Trace::default())
        .expect("generation");
    // Then semantic leaf callbacks receive numeric formulas and placement overrides only once.
    let text = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    assert!(text.contains("map-alignment:4|offset:0|extent:4"));
    assert_eq!(text.matches("short:Placement:control:placement").count(), 1);
    assert_eq!(text.matches("short:Register:Control:register").count(), 1);
    Ok(())
}
