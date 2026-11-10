//! Rendering to caller-selected writers does not require artifact metadata.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{generate, record, value},
    record_schema,
};
record_schema! { pub record Packet { endian:Little, fields:{value:u32} } }
struct Text;
impl value::Backend for Text {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;
}
impl record::Backend for Text {
    fn entry(&mut self, ctx: Self::Context<'_>, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        write!(w, "{}", ctx.entry_name())
    }
}
#[test]
fn ordinary_backend_generates_without_output_metadata() -> Result<(), Box<dyn std::error::Error>> {
    // Given a backend implementing only value and record callbacks.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut bytes = Vec::new();
    // When the caller supplies ordinary in-memory storage.
    generate(&schema, Text, &mut bytes)?;
    // Then generation needs no output identity/metadata protocol.
    assert_eq!(bytes, b"Packet");
    Ok(())
}
