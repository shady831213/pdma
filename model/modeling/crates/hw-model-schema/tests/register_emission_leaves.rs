//! Register leaf callbacks receive effective fields and owner-specific geometry.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::export::value::Emit as _;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
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
    Expression,
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
    fn reference<'a>(
        &mut self,
        _: export::view::Reference<'a>,
        metadata: export::view::Target<'a>,
        target: export::Visit<register::Context<'a>>,
        _: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        if metadata.kind == export::Kind::Bits {
            self.emit(target, writer)?;
        }
        Ok(())
    }
    fn register<'a>(
        &mut self,
        value: export::view::Register<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        assert!(
            ctx.register()
                .is_some_and(|register| register.source().is_some())
        );
        self.push(format!("width:{}", value.width()), writer)?;
        self.push(format!("register-alignment:{}", value.alignment()), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn map_layout<'a>(
        &mut self,
        value: export::view::MapLayout<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        self.push(format!("map-alignment:{}", value.alignment()), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn register_field<'a>(
        &mut self,
        field: hw_model_schema::export::view::BitField<'a, usize>,
        ctx: hw_model_schema::export::register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        self.push(format!("field:{}", field.name()), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn layout_property<'a>(
        &mut self,
        property: register::LayoutProperty<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        use hw_model_schema::ast::register::LayoutPropertyKind;
        if matches!(
            property.kind(),
            LayoutPropertyKind::Offset | LayoutPropertyKind::Extent
        ) {
            self.push(property.name().into(), writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for Trace {
    type Domain = register::Domain;
    type Error = Failure;
    type Context<'a> = hw_model_schema::export::register::Context<'a>;
    fn bit_field<'a>(
        &mut self,
        field: export::view::BitField<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        if ctx.register().is_some() {
            self.push(format!("field:{}", field.name()), writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn expression<'a>(
        &mut self,
        expression: hw_model_schema::export::view::Expression<'a>,
        _: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        let value = expression
            .evaluate(&[], &[])
            .map_err(|_| Failure::Expression)?;
        self.push(format!("value:{value}"), writer)
    }
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
}
impl hw_model_schema::export::OutputBackend for Trace {
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.root_name().to_owned(),
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
    // When shared recursion explicitly follows the register's bits reference.
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
    assert!(
        text.contains(
            "map-alignment:4|offset|value:0|value:1|value:4|value:4|value:4|extent|value:4"
        ),
        "{text}"
    );
    assert_eq!(text.matches("short:Placement:control:placement").count(), 1);
    assert_eq!(text.matches("short:Register:Control:register").count(), 1);
    Ok(())
}
