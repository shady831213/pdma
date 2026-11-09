//! Referenced value encoding retains its embedding's byte order.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    bits_schema,
    export::{
        OutputMetadata, record,
        value::{self, Emit as _},
        view,
    },
    record_schema,
};

bits_schema! { pub bits Flags(u16) { flag: 8..10 } }
record_schema! { pub record Big { endian: Big, fields: { flags: bits(Flags) } } }

struct Target;
impl record::Backend for Target {
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        target: hw_model_schema::export::Visit<record::Context<'a>>,
        _: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit(target, writer)
    }
}
impl value::Backend for Target {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;

    fn bits<'a>(
        &mut self,
        bits: view::Bits<'a>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let encoding = bits.encoding();
        writeln!(
            writer,
            "{}:{:?}:{}:{:?}:{}",
            ctx.path(),
            encoding.scalar(),
            encoding.width(),
            encoding.endian(),
            bits.bit_width()
        )
    }
}
impl hw_model_schema::export::OutputBackend for Target {
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: ctx.root_name().into(),
            media_type: "text/plain".into(),
        })
    }
}

#[test]
fn referenced_bits_receive_the_use_site_encoding() {
    // Given: reusable bits embedded in a big-endian record.
    let mut set = SchemaSet::new();
    set.add_record::<Big>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut output = Vec::new();
    // When: explicit reference expansion reaches the bits callback.
    Target
        .emit_all(
            record::roots(&schema).filter(|visit| visit.name() == Some("Big")),
            &mut output,
        )
        .unwrap();
    // Then: encoding combines declared scalar geometry with the actual placement.
    assert_eq!(output, b"flags:U16:2:Big:10\n");
}
