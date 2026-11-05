//! Referenced records keep declaration attributes distinct from placement attributes.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::export::value::Backend as _;
use hw_model_schema::{
    Endian,
    analysis::{AnalyzedSchema, Bindings},
    ast::{Field, SchemaSet, record::Node},
    export::{record, value},
    record_schema,
    traversal::record::Context,
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
    fn field<'a>(
        &mut self,
        _: &'a Field,
        ctx: Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)?;
        if let Some(child) = ctx.referent() {
            self.emit(child, writer)?;
        }
        Ok(())
    }
    fn record_size(
        &mut self,
        size: usize,
        ctx: Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let Some(Node::Record(record)) = ctx.ancestors().last() else {
            return Err("record attribute requires a record owner");
        };
        self.push(
            format!("{}:record:size:{size}", record.spec().name()),
            writer,
        )?;
        Ok(())
    }
    fn field_size(
        &mut self,
        size: usize,
        ctx: Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let Some(Node::Field(field)) = ctx.ancestors().last() else {
            return Err("field attribute requires a field owner");
        };
        self.push(format!("{}:field:size:{size}", field.name()), writer)?;
        Ok(())
    }
    fn record_endian(
        &mut self,
        endian: Endian,
        ctx: Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let Some(Node::Record(record)) = ctx.ancestors().last() else {
            return Err("record endian requires a record owner");
        };
        self.push(
            format!("{}:record:endian:{endian:?}", record.spec().name()),
            writer,
        )?;
        Ok(())
    }
    fn field_endian(
        &mut self,
        endian: Endian,
        ctx: Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let Some(Node::Field(field)) = ctx.ancestors().last() else {
            return Err("field endian requires a field owner");
        };
        self.push(format!("{}:field:endian:{endian:?}", field.name()), writer)?;
        Ok(())
    }
}
impl value::Backend for Owners {
    type Error = &'static str;
    type Context<'a> = Context<'a>;
    type Domain = record::Domain;
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
