//! Referenced records keep declaration attributes distinct from placement attributes.
use hw_model_schema::export::value::Backend as _;
use hw_model_schema::{
    Endian,
    analysis::{AnalyzedSchema, Bindings},
    ast::{Field, SchemaSet, record::Node},
    export::{Artifact, InMemoryArtifactSink, generate, record, value},
    record_schema,
    traversal::record::Context,
};

record_schema! { pub record Child { endian: Big, fields: { payload: u16 } } }
record_schema! { pub record Parent { endian: Little, fields: { child: Child } } }

#[derive(Default)]
struct Owners(Vec<String>);
impl record::Backend for Owners {
    fn field<'a>(&mut self, _: &'a Field, ctx: Context<'a>) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())?;
        if let Some(child) = ctx.referent() {
            self.emit(child)?;
        }
        Ok(())
    }
    fn record_size(&mut self, size: usize, ctx: Context<'_>) -> Result<(), Self::Error> {
        let Some(Node::Record(record)) = ctx.ancestors().last() else {
            return Err("record attribute requires a record owner");
        };
        self.0
            .push(format!("{}:record:size:{size}", record.spec().name()));
        Ok(())
    }
    fn field_size(&mut self, size: usize, ctx: Context<'_>) -> Result<(), Self::Error> {
        let Some(Node::Field(field)) = ctx.ancestors().last() else {
            return Err("field attribute requires a field owner");
        };
        self.0.push(format!("{}:field:size:{size}", field.name()));
        Ok(())
    }
    fn record_endian(&mut self, endian: Endian, ctx: Context<'_>) -> Result<(), Self::Error> {
        let Some(Node::Record(record)) = ctx.ancestors().last() else {
            return Err("record endian requires a record owner");
        };
        self.0
            .push(format!("{}:record:endian:{endian:?}", record.spec().name()));
        Ok(())
    }
    fn field_endian(&mut self, endian: Endian, ctx: Context<'_>) -> Result<(), Self::Error> {
        let Some(Node::Field(field)) = ctx.ancestors().last() else {
            return Err("field endian requires a field owner");
        };
        self.0
            .push(format!("{}:field:endian:{endian:?}", field.name()));
        Ok(())
    }
}
impl value::Backend for Owners {
    type Error = &'static str;
    type Context<'a> = Context<'a>;
    type Domain = record::Domain;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "owners".into(),
            media_type: "text/plain".into(),
            contents: self.0.join("\n").into_bytes(),
        }])
    }
}

#[test]
fn following_a_reference_keeps_record_and_field_attribute_owners_distinct() {
    let mut set = SchemaSet::new();
    set.add_record::<Parent>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut sink = InMemoryArtifactSink::new();
    generate(&schema, Owners::default(), &mut sink).unwrap();
    let output = String::from_utf8(sink.artifacts().first().unwrap().contents.clone()).unwrap();
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
