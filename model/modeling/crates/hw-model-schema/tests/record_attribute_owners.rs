//! Referenced records keep declaration attributes distinct from placement attributes.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::export::value::Emit as _;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::record::Context,
    export::{record, value},
    record_schema,
};
use output_support::MemoryOutput;

record_schema! { pub record Child { endian: Big, fields: { payload: u16 } } }
record_schema! { pub record Parent { endian: Little, fields: { child: Child } } }

#[derive(Default)]
struct Owners(Vec<String>);
impl Owners {
    fn push(&mut self, event: String, writer: &mut dyn std::io::Write) -> Result<(), &'static str> {
        if !self.0.is_empty() {
            writer.write_all(b"\n").map_err(|_| "writer rejected")?;
        }
        writer
            .write_all(event.as_bytes())
            .map_err(|_| "writer rejected")?;
        self.0.push(event);
        Ok(())
    }
}
impl record::Backend for Owners {
    fn reference<'a>(
        &mut self,
        _: hw_model_schema::export::view::Reference<'a>,
        target: hw_model_schema::export::Visit<Context<'a>>,
        _: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit(target, writer)
    }
    fn record<'a>(
        &mut self,
        record: hw_model_schema::export::view::Record<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.current().kind(), hw_model_schema::export::Kind::Record);
        let name = record.spec().name();
        self.push(
            format!("{name}:record:size:{}", record.spec().size()),
            writer,
        )?;
        self.push(
            format!("{name}:record:endian:{:?}", record.endian()),
            writer,
        )?;
        self.emit_all(ctx.children(), writer)
    }
    fn field<'a>(
        &mut self,
        field: hw_model_schema::export::view::Field<'a>,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        assert_eq!(ctx.current().kind(), hw_model_schema::export::Kind::Field);
        assert_eq!(
            ctx.field().ok_or("missing field owner")?.name(),
            field.name()
        );
        self.push(
            format!("{}:field:size:{}", field.name(), field.width()),
            writer,
        )?;
        self.push(
            format!("{}:field:endian:{:?}", field.name(), field.endian()),
            writer,
        )?;
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for Owners {
    type Error = &'static str;
    type Context<'a> = Context<'a>;
    type Domain = record::Domain;
}
impl hw_model_schema::export::OutputBackend for Owners {
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

#[test]
fn following_a_reference_keeps_record_and_field_attribute_owners_distinct() {
    let mut set = SchemaSet::new();
    set.add_record::<Parent>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut sink = MemoryOutput::new();
    sink.generate(&schema, Owners::default()).unwrap();
    let output = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(
        lines,
        [
            "Child:record:size:2",
            "Child:record:endian:Big",
            "payload:field:size:2",
            "payload:field:endian:Big",
            "Parent:record:size:2",
            "Parent:record:endian:Little",
            "child:field:size:2",
            "child:field:endian:Little",
            "Child:record:size:2",
            "Child:record:endian:Big",
            "payload:field:size:2",
            "payload:field:endian:Big",
        ]
    );
}
