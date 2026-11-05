use super::*;
use crate::output_support::MemoryOutput;
use hw_model_schema::export::value::Backend as _;
#[derive(Default)]
struct Documentation(Vec<String>);
impl hw_model_schema::export::value::Backend for Documentation {
    type Error = std::io::Error;
    type Context<'a> = hw_model_schema::traversal::register::Context<'a>;
    type Domain = hw_model_schema::export::register::Domain;
    fn expression<'a>(
        &mut self,
        value: &'a LayoutExpr,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("expression:{value:?}");
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        self.emit_all(ctx.children(), writer)
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
impl hw_model_schema::export::register::Backend for Documentation {
    fn parameter(
        &mut self,
        value: ast::register::Parameter,
        _: hw_model_schema::traversal::register::Context<'_>,
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
        value: ast::register::Argument<'a>,
        ctx: hw_model_schema::traversal::register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("binding:{:?}", value.parameter);
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        self.emit_all(ctx.children(), writer)
    }
    fn register_width(
        &mut self,
        value: usize,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("attribute:Width({value})");
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        Ok(())
    }
    fn register_access(
        &mut self,
        value: hw_model_schema::Access,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("attribute:Access({value:?})");
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        Ok(())
    }
    fn register_reset(
        &mut self,
        kind: hw_model_schema::ResetKind,
        value: u64,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("attribute:Reset({kind:?}, {value})");
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        Ok(())
    }
    fn register_writable_mask(
        &mut self,
        value: u64,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = format!("attribute:WritableMask({value})");
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        Ok(())
    }
    fn layout_stride<'a>(
        &mut self,
        _value: &'a LayoutExpr,
        ctx: hw_model_schema::traversal::register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = "layout:stride".into();
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
        "expression:Parameter(\"count\")",
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
