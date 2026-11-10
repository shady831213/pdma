//! Each logical field exposes its semantic value to shared dispatch.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{record, value, view},
    record_schema,
};
use value::Emit as _;

record_schema! { pub record Values { endian: Little, fields: {
    plain: u32,
    choice: enum(u16) { variants: { Off = 0, On = 1 } },
    flags: bits(u8) { fields: { flag: 0..1 => enum { variants: { Clear = 0, Set = 1 } } } },
} } }
record_schema! { pub record References { endian: Little, fields: { first: Values, second: Values } } }

#[derive(Default)]
struct Probe(Vec<String>);
impl record::Backend for Probe {
    fn field<'a>(
        &mut self,
        field: view::Field<'a>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        for visit in ctx
            .children()
            .filter(|v| v.kind() != hw_model_schema::export::Kind::Attribute)
        {
            self.0.push(format!("{}:{:?}", field.name(), visit.kind()));
            self.emit(visit, writer)?;
        }
        Ok(())
    }
}
impl value::Backend for Probe {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
}

#[test]
fn every_field_value_has_a_bound_semantic_visit() {
    // Given: scalar, enum and bits fields in the same record.
    let mut set = SchemaSet::new();
    set.add_record::<Values>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut probe = Probe::default();
    // When: a target selects each field's value visits through shared dispatch.
    probe
        .emit_all(record::entries(&schema), &mut std::io::sink())
        .unwrap();
    // Then: ordinary scalars are values too, with no parent type inspection.
    assert_eq!(
        probe.0,
        ["plain:Scalar", "choice:Reference", "flags:Reference"]
    );
}

struct EncodingTarget {
    expand: bool,
}
impl record::Backend for EncodingTarget {
    fn reference<'a>(
        &mut self,
        reference: view::Reference<'a>,
        target: hw_model_schema::export::Visit<record::Context<'a>>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if target.definition_origin() == hw_model_schema::ast::Origin::Inline {
            return self.emit(target, writer);
        }
        if self.expand {
            self.emit(target.clone(), writer)?;
            self.emit(target, writer)
        } else {
            writeln!(writer, "reference:{}:{}", ctx.path(), reference.name())
        }
    }
    fn constant(
        &mut self,
        value: &hw_model_schema::ast::Constant,
        _: record::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        writeln!(writer, "constant:{}:{}", value.symbol, value.value)
    }
}
impl value::Backend for EncodingTarget {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;

    fn scalar(
        &mut self,
        value: view::Scalar,
        ctx: record::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        writeln!(
            writer,
            "scalar:{}:{:?}:{}:{:?}",
            ctx.path(),
            value.scalar(),
            value.width(),
            value.endian()
        )
    }
    fn enumeration(
        &mut self,
        value: view::Enumeration,
        ctx: record::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        match value.encoding() {
            view::EnumEncoding::Scalar(scalar) => writeln!(
                writer,
                "enum:{}:{:?}:{}:{:?}",
                ctx.path(),
                scalar.scalar(),
                scalar.width(),
                scalar.endian()
            )?,
            view::EnumEncoding::Bits { bit_width } => writeln!(
                writer,
                "packed:{}:{}:{bit_width}",
                ctx.path(),
                ctx.bit()
                    .ok_or_else(|| std::io::Error::other("packed enum missing bit placement"))?
            )?,
        }
        self.emit_all(ctx.children(), writer)
    }
    fn bits<'a>(
        &mut self,
        value: view::Bits<'a>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let scalar = value.encoding();
        writeln!(
            writer,
            "bits:{}:{:?}:{}:{:?}",
            ctx.path(),
            scalar.scalar(),
            scalar.width(),
            scalar.endian()
        )?;
        self.emit_all(ctx.children(), writer)
    }
}

#[test]
fn value_callbacks_receive_complete_scalar_and_enum_encodings() {
    // Given: byte and packed enums beside an ordinary scalar.
    let mut set = SchemaSet::new();
    set.add_record::<Values>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut output = Vec::new();
    // When: the target uses default field traversal and only value callbacks.
    EncodingTarget { expand: true }
        .emit_all(record::entries(&schema), &mut output)
        .unwrap();
    // Then: callbacks receive their own width, byte order, and packed range.
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "scalar:plain:U32:4:Little\nenum:choice:U16:2:Little\nbits:flags:U8:1:Little\npacked:flags:flag:1\n"
    );
}

#[test]
fn scalar_constant_preserves_constraint_and_encoding_callbacks() {
    use hw_model_schema::{Endian, FieldDefinition, RecordSpec, ScalarKind, ast};
    // Given: a big-endian scalar constrained by a source constant.
    let scalar = FieldDefinition::new(ScalarKind::U16).with_endian(Endian::Big);
    let field = ast::Field::new(
        "magic",
        0,
        2,
        Endian::Big,
        ast::FieldType::Scalar {
            definition: scalar,
            constant: Some(ast::Constant {
                symbol: "MAGIC",
                value: 7,
            }),
        },
    );
    let schema = AnalyzedSchema::record(
        "constant",
        ast::RecordDefinition::new(RecordSpec::new(2, 2, &[]), vec![field]),
    )
    .unwrap();
    let mut output = Vec::new();
    // When: default field traversal dispatches both source constraint and value.
    EncodingTarget { expand: true }
        .emit_all(record::entries(&schema), &mut output)
        .unwrap();
    // Then: the constraint is not lost and scalar encoding remains independent.
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "constant:MAGIC:7\nscalar:magic:U16:2:Big\n"
    );
}

#[test]
fn reference_targets_support_symbolic_output_without_expansion() {
    // Given: two references to the same definition.
    let mut set = SchemaSet::new();
    set.add_record::<References>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut output = Vec::new();
    // When: the reference callback chooses symbolic output.
    EncodingTarget { expand: false }
        .emit_all(
            record::entries(&schema).filter(|v| v.name() == Some("References")),
            &mut output,
        )
        .unwrap();
    // Then: only the two reference occurrences produce output.
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "reference:first:first\nreference:second:second\n"
    );
}

#[test]
fn repeated_reference_expansion_preserves_each_use_site() {
    // Given: two references whose target visits can be dispatched repeatedly.
    let mut set = SchemaSet::new();
    set.add_record::<References>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut output = Vec::new();
    // When: the reference callback expands each retained target twice.
    EncodingTarget { expand: true }
        .emit_all(
            record::entries(&schema).filter(|v| v.name() == Some("References")),
            &mut output,
        )
        .unwrap();
    // Then: all repeated scalar callbacks keep their original use-site paths.
    let text = String::from_utf8(output).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("scalar:"))
            .collect::<Vec<_>>(),
        [
            "scalar:first.plain:U32:4:Little",
            "scalar:first.plain:U32:4:Little",
            "scalar:second.plain:U32:4:Little",
            "scalar:second.plain:U32:4:Little"
        ]
    );
}
