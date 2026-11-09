//! Shallow ancestry preserves containment without exposing source nodes.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{Kind, record, register, value, view},
    record_schema, register_schema,
};
use std::io::{self, Write};
use value::Emit as _;

record_schema! { pub record Packet { endian:Little, fields:{
    tag:enum(u8){variants:{One=1}},
    payload:tagged_union{tag:self.tag,cases:{One{data:u8}}},
} } }
register_schema! { pub register Status { width:4,endian:Little,access:ReadOnly,
    reset:Zero(0),writable_mask:0,fields:{ready:0..=0},
} }

struct Records;
impl value::Backend for Records {
    type Domain = record::Domain;
    type Error = io::Error;
    type Context<'a> = record::Context<'a>;
}
impl record::Backend for Records {
    fn field<'a>(
        &mut self,
        field: view::Field<'a>,
        ctx: record::Context<'a>,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        let nested = ctx.ancestor_kinds().any(|kind| kind == Kind::Case);
        write!(writer, "{}:{nested};", field.name())?;
        self.emit_all(ctx.children(), writer)
    }
}

struct Registers;
impl value::Backend for Registers {
    type Domain = register::Domain;
    type Error = io::Error;
    type Context<'a> = register::Context<'a>;
}
impl register::Backend for Registers {
    fn register_field<'a>(
        &mut self,
        field: view::BitField<'a, usize>,
        ctx: register::Context<'a>,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        let kinds: Vec<_> = ctx.ancestor_kinds().collect();
        assert_eq!(kinds, [Kind::Root, Kind::Register]);
        writer.write_all(field.name().as_bytes())
    }
}

#[test]
fn containment_ancestry_distinguishes_inline_case_fields() -> Result<(), Box<dyn std::error::Error>>
{
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut output = Vec::new();
    Records.emit_all(record::roots(&schema), &mut output)?;
    assert_eq!(output, b"tag:false;payload:false;data:true;");
    Ok(())
}

#[test]
fn register_ancestry_preserves_outer_to_inner_order() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Status>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut output = Vec::new();
    Registers.emit_all(register::roots(&schema), &mut output)?;
    assert_eq!(output, b"ready");
    Ok(())
}
