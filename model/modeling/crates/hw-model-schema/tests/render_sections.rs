//! Local rendering sections preserve bound instances and writer errors.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{self, register, value, view},
    register_map, register_schema,
};
use value::Emit as _;

register_schema! { pub register Word {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:0xffff_ffff,fields:{}} }
register_map! { pub map Block(n:usize) {alignment:4,placements:[pub words:Word[n]]} }

#[derive(Default)]
struct Listing {
    values: Vec<u8>,
}
impl value::Backend for Listing {
    type Domain = register::Domain;
    type Error = std::io::Error;
    type Context<'a> = register::Context<'a>;
    fn expression<'a>(
        &mut self,
        expression: view::Expression<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let number = expression
            .evaluate(ctx.declared_parameters(), ctx.bindings().unwrap_or(&[]))
            .map_err(std::io::Error::other)?;
        write!(writer, "{number}")
    }
}

impl register::Backend for Listing {
    fn register_map<'a>(
        &mut self,
        _: view::Map<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.values.clear();
        write!(writer, "{}[", ctx.entry_name())?;
        self.emit_all(ctx.children(), writer)?;
        writer.write_all(&self.values)?;
        writer.write_all(b"]")
    }
    fn layout_property<'a>(
        &mut self,
        property: register::LayoutProperty<'a>,
        ctx: register::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        if property.kind() == hw_model_schema::traversal::register::LayoutPropertyKind::Count
            && ctx.bank().is_none()
        {
            let owner = property.owner().ok_or(std::io::ErrorKind::InvalidInput)?;
            write!(writer, "{owner}:")?;
            let mut value = Vec::new();
            self.emit_all(ctx.children(), &mut value)?;
            self.values.extend(value);
        }
        Ok(())
    }
}

fn schema() -> Result<AnalyzedSchema, Box<dyn std::error::Error>> {
    let mut set = SchemaSet::new();
    set.add_map_instance::<Block>("small", &[2])?;
    set.add_map_instance::<Block>("large", &[7])?;
    Ok(AnalyzedSchema::analyze(set, &Bindings::new())?)
}

#[test]
fn render_sections_preserve_bindings_and_unit_local_order() {
    // Given two named instances and a renderer with local output sections.
    let schema = schema().unwrap();
    let roots = register::entries(&schema).filter(|root| root.id().view_name().is_some());
    let prepared = export::prepare_entries(&mut Listing::default(), roots).unwrap();
    let mut bytes = Vec::new();
    // When each instance is traversed once before flushing its value section.
    for output in prepared {
        output.emit(&mut Listing::default(), &mut bytes).unwrap();
    }
    // Then lexical bindings stay independent and output stays inside its unit.
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "large[words:7]small[words:2]"
    );
}

#[derive(Default)]
struct RejectValue(Vec<u8>);
impl std::io::Write for RejectValue {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.iter().any(u8::is_ascii_digit) {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn buffered_section_writer_failure_stops_before_closing_or_next_unit() {
    // Given a destination rejecting the buffered numeric output.
    let schema = schema().unwrap();
    let roots = register::entries(&schema).filter(|root| root.id().view_name().is_some());
    let prepared = export::prepare_entries(&mut Listing::default(), roots).unwrap();
    let mut writer = RejectValue::default();
    // When prepared outputs are emitted until the first writer failure.
    let result = prepared
        .iter()
        .try_for_each(|output| output.emit(&mut Listing::default(), &mut writer));
    // Then the original writer error escapes and no subsequent syntax is emitted.
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::BrokenPipe);
    assert_eq!(writer.0, b"large[words:");
}
