//! Structural defaults recurse; unhandled leaf syntax and named references remain silent.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{record, register, value, view},
    record_schema, register_map, register_schema,
};
use value::Emit as _;

register_schema! { pub register Word {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:0xffff_ffff,fields:{}} }
register_map! { pub map Block(n:usize) {alignment:4,placements:[pub words:Word[n]]} }
record_schema! { pub record Payload { endian: Little, fields: { value: u32 } } }
record_schema! { pub record Packet { endian: Little, fields: { payload: Payload } } }

struct Expressions;
impl register::Backend for Expressions {}
impl value::Backend for Expressions {
    type Domain = register::Domain;
    type Error = std::io::Error;
    type Context<'a> = register::Context<'a>;
}

struct Records;
impl record::Backend for Records {
    fn record<'a>(
        &mut self,
        node: view::Record<'a>,
        ctx: record::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        writeln!(writer, "{}", node.spec().name())?;
        self.emit_all(ctx.children(), writer)
    }
}
impl value::Backend for Records {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
}

struct ExpressionLeaves;
impl register::Backend for ExpressionLeaves {}
impl value::Backend for ExpressionLeaves {
    type Domain = register::Domain;
    type Error = std::io::Error;
    type Context<'a> = register::Context<'a>;
    fn expression<'a>(
        &mut self,
        _: view::Expression<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if ctx.children().next().is_some() {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        writer.write_all(b"expression")
    }
}

#[test]
fn symbolic_expressions_are_leaf_render_visits() {
    // Given a symbolic map whose completed extent contains arithmetic operands.
    let mut set = SchemaSet::new();
    set.add::<Block>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut bytes = Vec::new();
    // When a backend inherits structural traversal and only implements the expression leaf.
    ExpressionLeaves
        .emit_all(register::entries(&schema), &mut bytes)
        .unwrap();
    // Then expression callbacks run without exposing recursive operand dispatch.
    assert!(!bytes.is_empty());
}

#[test]
fn unhandled_leaf_syntax_does_not_emit_output() {
    // Given a backend implementing structural recursion but no expression syntax.
    let mut set = SchemaSet::new();
    set.add::<Block>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut bytes = Vec::new();
    // When structural defaults visit all selected units.
    Expressions
        .emit_all(register::entries(&schema), &mut bytes)
        .unwrap();
    // Then no unrequested expression syntax leaks into the artifact.
    assert!(bytes.is_empty(), "{bytes:?}");
}

#[test]
fn unhandled_record_references_do_not_duplicate_definitions() {
    // Given selected dependency roots and a backend emitting only record declarations.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut bytes = Vec::new();
    // When the containment defaults encounter a reusable record reference.
    Records
        .emit_all(record::entries(&schema), &mut bytes)
        .unwrap();
    // Then each definition is emitted exactly once, in dependency order.
    assert_eq!(String::from_utf8(bytes).unwrap(), "Payload\nPacket\n");
}
