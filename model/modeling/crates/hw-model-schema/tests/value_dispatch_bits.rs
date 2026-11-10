//! Referenced value encoding retains its embedding's byte order.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    bits_schema,
    export::{
        record,
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

    fn enumeration(
        &mut self,
        value: view::Enumeration,
        ctx: record::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if let view::EnumEncoding::Scalar(encoding) = value.encoding() {
            writeln!(writer, "{}:{:?}", ctx.path(), encoding.endian())?;
        }
        Ok(())
    }
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
            record::entries(&schema).filter(|visit| visit.name() == Some("Big")),
            &mut output,
        )
        .unwrap();
    // Then: encoding combines declared scalar geometry with the actual placement.
    assert_eq!(output, b"flags:U16:2:Big:10\n");
}

#[test]
fn referenced_enum_uses_each_placement_byte_order() -> Result<(), Box<dyn std::error::Error>> {
    use hw_model_schema::{
        Documentation, Endian, EnumDefinition, EnumVariantSpec, FieldDefinition, RecordSpec,
        ScalarKind, ast::*,
    };
    struct Code;
    fn enumeration() -> Definition {
        static VARIANTS: [EnumVariantSpec; 1] = [EnumVariantSpec::new("Present", 0x1234)];
        Definition::from_body(
            DefinitionId::of::<Code>(),
            "Code".into(),
            Documentation::EMPTY,
            DefinitionBody::Enumeration(value::Enumeration::Scalar(
                FieldDefinition::new(ScalarKind::U16).with_enum(EnumDefinition::new(&VARIANTS)),
            )),
        )
    }
    let fields = [("big", 0, Endian::Big), ("little", 2, Endian::Little)]
        .into_iter()
        .map(|(name, offset, endian)| {
            Field::new(
                name,
                offset,
                2,
                endian,
                FieldType::Reference(Reference::new(name, ReferenceKind::Field, enumeration)),
            )
        })
        .collect();
    let schema = AnalyzedSchema::record(
        "Endian",
        RecordDefinition::new(RecordSpec::named("Endian", 4, 2, &[], &[]), fields),
    )?;
    let mut output = Vec::new();
    Target.emit_all(
        hw_model_schema::export::record::entries(&schema).filter(|v| v.name() == Some("Endian")),
        &mut output,
    )?;
    assert_eq!(output, b"big:Big\nlittle:Little\n");
    Ok(())
}
