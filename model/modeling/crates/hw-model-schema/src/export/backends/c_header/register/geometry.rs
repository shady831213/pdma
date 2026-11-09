use super::{CRegisterBackend, CResult, Context, Fragment, symbols};
use crate::export::{register::LayoutProperty, value::Emit as _};
use std::io::Write;

#[derive(Debug, Default)]
pub(in super::super) struct Geometry {
    names: symbols::Scope,
    members: Vec<u8>,
    assignments: Vec<u8>,
}

impl CRegisterBackend {
    pub(super) fn emit_geometry(&mut self, ctx: &Context<'_>, writer: &mut dyn Write) -> CResult {
        let mut names = symbols::GeometryNames::new(&self.prefix)?;
        self.geometry = Geometry::default();
        self.emit_all(ctx.children(), writer)?;
        Fragment::line("typedef struct {", writer)?;
        Fragment::line("    uint64_t extent;", writer)?;
        writer.write_all(&self.geometry.members)?;
        Fragment::line(format_args!("}} {}_Geometry;", self.prefix), writer)?;
        write!(
            writer,
            "static inline {0}_Geometry {0}_GEOMETRY(",
            self.prefix
        )?;
        let mut unused = Vec::new();
        for (index, parameter) in ctx.parameters().iter().enumerate() {
            let name = format!("p_{}", symbols::c_identifier(parameter));
            names.parameters.insert(&name, &name)?;
            if index != 0 {
                writer.write_all(b", ")?;
            }
            write!(writer, "uint64_t {name}")?;
            writeln!(unused, "    (void){name};")?;
        }
        if ctx.parameters().is_empty() {
            writer.write_all(b"void")?;
        }
        writeln!(writer, ") {{\n    {}_Geometry result = {{0}};", self.prefix)?;
        writer.write_all(&unused)?;
        writer.write_all(&self.geometry.assignments)?;
        Fragment::line("    return result;\n}", writer)
    }

    pub(super) fn geometry_property(
        &mut self,
        property: LayoutProperty<'_>,
        ctx: &Context<'_>,
    ) -> CResult {
        let name = property.owner().map_or_else(
            || property.name().into(),
            |owner| format!("{}_{}", symbols::c_identifier(owner), property.name()),
        );
        if property.owner().is_some() {
            self.geometry.names.insert(&name, &name)?;
            writeln!(self.geometry.members, "    uint64_t {name};")?;
        }
        let mut expression = Vec::new();
        self.emit_all(ctx.children(), &mut expression)?;
        write!(self.geometry.assignments, "    result.{name} = ")?;
        self.geometry.assignments.write_all(&expression)?;
        writeln!(self.geometry.assignments, ";")?;
        Ok(())
    }
}
