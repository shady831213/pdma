//! A value renderer handles declared register fields and reused bits through one callback.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::{Definition, SchemaSet},
    bits_schema,
    export::{
        Visit, register,
        value::{self, Emit},
        view,
    },
    register_schema,
};
bits_schema! { pub bits Flags(u32) {
    mode:0..=1 => enum {variants:{Idle=0,Active=2}}, ready:8..=8,
} }
register_schema! {pub register Control {width:4,endian:Little,access:ReadWrite,
reset:Constant(2),writable_mask:3,fields:Flags}}
struct Fields;
impl register::Backend for Fields {
    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        _: view::Target<'a>,
        target: Visit<register::Context<'a>>,
        _: register::Context<'a>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        self.emit(target, w)
    }
}
impl value::Backend for Fields {
    type Domain = register::Domain;
    type Error = std::io::Error;
    type Context<'a> = register::Context<'a>;
    fn bit_field(
        &mut self,
        field: view::BitField,
        ctx: Self::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        let owner = ctx
            .register()
            .ok_or_else(|| std::io::Error::other("field owner"))?;
        writeln!(
            w,
            "{}:{:?}:{}:{}",
            field.name(),
            field.range(),
            owner.reset_value(),
            owner.writable_mask()
        )?;
        self.emit_all(ctx.children(), w)
    }
    fn variant(
        &mut self,
        v: &hw_model_schema::EnumVariantSpec,
        _: Self::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(w, "{}={}", v.name(), v.value())
    }
}
#[test]
fn default_register_field_dispatch_preserves_owner_and_enum_children()
-> Result<(), Box<dyn std::error::Error>> {
    // Given equivalent named-bits and directly declared register fields.
    let mut named = SchemaSet::new();
    named.add::<Control>();
    let mut direct = SchemaSet::new();
    direct.add_definition(Definition::register::<Control, _>(Control::SPEC));
    for source in [named, direct] {
        let schema = AnalyzedSchema::analyze(source, &Bindings::new())?;
        let mut bytes = Vec::new();
        // When only the value-level bit_field callback is implemented.
        Fields.emit_all(
            register::entries(&schema).filter(|v| v.name() == Some("Control")),
            &mut bytes,
        )?;
        // Then both embeddings expose one field callback, correct ownership and enum recursion.
        assert_eq!(
            String::from_utf8(bytes)?,
            "mode:Some(0..2):2:3\nIdle=0\nActive=2\nready:Some(8..9):2:3\n"
        );
    }
    Ok(())
}
