use super::*;
use hw_model_schema::export::value::Backend as _;
#[derive(Default)]
struct Documentation(Vec<String>);
impl hw_model_schema::export::value::Backend for Documentation {
    type Error = std::convert::Infallible;
    type Context<'a> = hw_model_schema::export::register::EmitContext<'a>;
    type Domain = hw_model_schema::export::register::Domain;
    fn expression<'a>(
        &mut self,
        value: &'a LayoutExpr,
        ctx: Self::Context<'a>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("expression:{value:?}"));
        self.emit_all(ctx.children())
    }
    fn finish(self) -> Result<Vec<hw_model_schema::export::Artifact>, Self::Error> {
        Ok(vec![hw_model_schema::export::Artifact {
            logical_name: "schema.txt".into(),
            media_type: "text/plain".into(),
            contents: self.0.join("\n").into_bytes(),
        }])
    }
}
impl hw_model_schema::export::register::Backend for Documentation {
    fn parameter(
        &mut self,
        value: ast::register::Parameter,
        _: hw_model_schema::export::register::EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("parameter:{}", value.name));
        Ok(())
    }
    fn argument<'a>(
        &mut self,
        value: ast::register::Argument<'a>,
        ctx: hw_model_schema::export::register::EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("binding:{:?}", value.parameter));
        self.emit_all(ctx.children())
    }
    fn register_width(&mut self, value: usize, _: Self::Context<'_>) -> Result<(), Self::Error> {
        self.0.push(format!("attribute:Width({value})"));
        Ok(())
    }
    fn register_access(
        &mut self,
        value: hw_model_schema::Access,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("attribute:Access({value:?})"));
        Ok(())
    }
    fn register_reset(
        &mut self,
        kind: hw_model_schema::ResetKind,
        value: u64,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("attribute:Reset({kind:?}, {value})"));
        Ok(())
    }
    fn register_writable_mask(
        &mut self,
        value: u64,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.0.push(format!("attribute:WritableMask({value})"));
        Ok(())
    }
    fn layout_stride<'a>(
        &mut self,
        _value: &'a LayoutExpr,
        ctx: hw_model_schema::export::register::EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.0.push("layout:stride".into());
        self.emit_all(ctx.children())
    }
}
#[test]
fn documentation_backend_sees_semantics_without_target_code_types()
-> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let analysis = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = hw_model_schema::export::InMemoryArtifactSink::new();
    hw_model_schema::export::generate(&analysis, Documentation::default(), &mut sink)?;
    let text = std::str::from_utf8(&sink.artifacts().first().ok_or("missing artifact")?.contents)?;
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
