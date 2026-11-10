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
    fn register_map<'a>(
        &mut self,
        value: export::view::Map<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Failure> {
        self.push(
            format!(
                "map-alignment:{}",
                value.alignment().ok_or(Failure::Description)?
            ),
            writer,
        )?;
        self.emit_all(ctx.children(), writer)
    }
    fn register_field(
        &mut self,
        field: hw_model_schema::export::view::BitField,
        ctx: hw_model_schema::export::register::Context<'_>,
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
    fn bit_field(
        &mut self,
        field: export::view::BitField,
        ctx: register::Context<'_>,
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

fn schema() -> Result<AnalyzedSchema, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Control>();
    Ok(AnalyzedSchema::from_graph(
        &set.resolve()?,
        &Bindings::new(),
    )?)
}
mod register_emission_cases;
