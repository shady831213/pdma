//! Structural defaults recurse; a target only implements actual leaf syntax.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{
        record, register,
        value::{self, Emit},
        view,
    },
    record_schema, register_map, register_schema,
};
register_schema! {pub register Word {width:4,endian:Little,access:ReadOnly,reset:Zero(0),writable_mask:0,fields:{}}}
register_map! {pub map Child(n:usize) {alignment:4,placements:[pub words:Word[n]]}}
register_map! {pub map Parent(n:usize) {alignment:4,placements:[pub child:Child(n)]}}
struct Expressions;
impl register::Backend for Expressions {}
impl value::Backend for Expressions {
    type Domain = register::Domain;
    type Error = std::io::Error;
    type Context<'a> = register::Context<'a>;
    fn expression(
        &mut self,
        e: view::Expression<'_>,
        ctx: Self::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(
            w,
            "{:?}:{}",
            ctx.parent_kind(),
            e.evaluate(&["n"], &[2]).map_err(std::io::Error::other)?
        )
    }
}
#[test]
fn defaults_reach_argument_and_layout_leaves() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Parent>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    Expressions.emit_all(
        register::entries(&schema).filter(|v| v.name() == Some("Parent")),
        &mut bytes,
    )?;
    let text = String::from_utf8(bytes)?;
    assert!(text.contains("Some(Argument):2\n"), "{text}");
    assert!(text.contains("Some(LayoutProperty):8\n"), "{text}");
    Ok(())
}
record_schema! {#[short_description="brief"] #[description="detail"] pub record Documented {endian:Little,fields:{value:u32}}}
struct Comments;
impl record::Backend for Comments {}
impl value::Backend for Comments {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
    fn short_description(
        &mut self,
        t: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        w: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(w, "{t}")
    }
}
#[test]
fn detailed_docs_default_to_the_same_target_format() -> Result<(), Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add::<Documented>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    Comments.emit_all(record::entries(&schema), &mut bytes)?;
    assert_eq!(bytes, b"brief\ndetail\n");
    Ok(())
}
