use super::*;
use crate::output_support::MemoryOutput;
use hw_model_schema::export::value::Emit as _;
struct SemanticSyntax;
impl hw_model_schema::export::expression::ExpressionSyntax for SemanticSyntax {}
#[derive(Default)]
struct Documentation(Vec<String>);
impl hw_model_schema::export::value::Backend for Documentation {
    type Error = std::io::Error;
    type Context<'a> = hw_model_schema::export::register::Context<'a>;
    type Domain = hw_model_schema::export::register::Domain;
    fn expression<'a>(
        &mut self,
        value: hw_model_schema::export::view::Expression<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!(
                "expression:{}",
                value
                    .format(&mut SemanticSyntax)
                    .map_err(std::io::Error::other)?
            );
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        self.emit_all(ctx.children(), writer)
    }
}

impl hw_model_schema::export::register::Backend for Documentation {
    fn parameter(
        &mut self,
        value: ast::register::Parameter,
        _: hw_model_schema::export::register::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("parameter:{}", value.name);
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        Ok(())
    }
    fn argument<'a>(
        &mut self,
        value: hw_model_schema::export::view::Argument<'a>,
        ctx: hw_model_schema::export::register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("binding:{:?}", value.parameter());
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        self.emit_all(ctx.children(), writer)
    }
    fn register<'a>(
        &mut self,
        value: hw_model_schema::export::view::Register<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        for event in [
            format!("attribute:Width({})", value.width()),
            format!("attribute:Access({:?})", value.access()),
            format!(
                "attribute:Reset({:?}, {})",
                value.reset_kind(),
                value.reset_value()
            ),
            format!("attribute:WritableMask({})", value.writable_mask()),
        ] {
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        self.emit_all(ctx.children(), writer)
    }
    fn layout_property<'a>(
        &mut self,
        property: hw_model_schema::export::register::LayoutProperty<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if property.kind() == ast::register::LayoutPropertyKind::Stride {
            let event = "layout:stride".to_owned();
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        self.emit_all(ctx.children(), writer)
    }
}
#[test]
fn documentation_backend_sees_semantics_without_target_code_types()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let analysis = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = MemoryOutput::new();
    sink.generate(&analysis, Documentation::default())?;
    let text = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    for required in [
        "parameter:count",
        "binding:Some(\"words\")",
        "expression:count",
        "layout:stride",
        "attribute:Width(4)",
        "attribute:Access(ReadWrite)",
        "attribute:Reset",
        "attribute:WritableMask(4294967295)",
    ] {
        assert!(text.contains(required), "missing {required}: {text}");
    }
    Ok(())
}
