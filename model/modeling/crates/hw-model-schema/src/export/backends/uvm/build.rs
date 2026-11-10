//! Renders child construction statements inside the generated UVM `build()` method.
use super::UvmRegisterBackend;
use crate::{
    ast::register::LayoutPropertyKind as Layout,
    export::{Kind, register::Context, value::Emit, view},
};
use std::io::{Error, Write};
impl UvmRegisterBackend {
    pub(super) fn emit_member_build(
        &mut self,
        member: &str,
        target: view::Target<'_>,
        ctx: &Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        let p = ctx
            .placement()
            .ok_or_else(|| Error::other("reference without placement"))?;
        let offset = self.layout_expression(ctx, Layout::Offset)?;
        let (handle, offset, instance) = if p.is_bank() {
            let count = self.layout_expression(ctx, Layout::Count)?;
            let stride = self.layout_expression(ctx, Layout::Stride)?;
            writeln!(
                writer,
                "      if ({count} == 0) `uvm_fatal(\"RAL_BANK_COUNT\", \"{member} count must be positive\")"
            )?;
            writeln!(
                writer,
                "      if ({count} > 64'd2147483647) `uvm_fatal(\"RAL_BANK_COUNT\", \"bank count exceeds target array range\")"
            )?;
            writeln!(
                writer,
                "      this.{member} = new[{count}];\n      foreach (this.{member}[i]) begin"
            )?;
            (
                format!("this.{member}[i]"),
                format!("({offset} + i * {stride})"),
                format!("$sformatf(\"{member}[%0d]\", i)"),
            )
        } else {
            (format!("this.{member}"), offset, format!("\"{member}\""))
        };
        writeln!(
            writer,
            "      {handle} = {}::type_id::create({instance});\n      {handle}.configure(this);",
            self.names.get(target.id, target.name)
        )?;
        if target.kind == Kind::Register {
            let access = super::fields::access(
                target
                    .access
                    .ok_or_else(|| Error::other("register target has no access rights"))?,
            );
            writeln!(
                writer,
                "      {handle}.build();\n      default_map.add_reg({handle}, {offset}, \"{access}\");"
            )?;
        } else {
            write!(writer, "      {handle}.build(")?;
            self.emit_all(ctx.arguments(), writer)?;
            writeln!(
                writer,
                "bus_bytes);\n      default_map.add_submap({handle}.default_map, {offset});"
            )?;
        }
        if p.is_bank() {
            writeln!(writer, "      end")?;
        }
        Ok(())
    }

    fn layout_expression(&mut self, ctx: &Context<'_>, kind: Layout) -> Result<String, Error> {
        let visit = ctx
            .layout(kind)
            .ok_or_else(|| Error::other("missing layout property"))?;
        let mut bytes = Vec::new();
        self.emit(visit, &mut bytes)?;
        String::from_utf8(bytes).map_err(Error::other)
    }
}
