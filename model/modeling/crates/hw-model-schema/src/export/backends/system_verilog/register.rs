use super::super::layout::register_name;
use super::SvRegisterBackend;
mod package;
mod values;
use crate::export::register::{Context, LayoutProperty};
use crate::{
    Endian,
    ast::RegisterLocation,
    export::{
        expression::ExpressionSyntax,
        register::Backend,
        value::Emit as _,
        view::{Map, Register},
    },
};
pub(super) use package::Output;
use std::io::Write;

impl Backend for SvRegisterBackend {
    fn reference<'a>(
        &mut self,
        _: crate::export::view::Reference<'a>,
        metadata: crate::export::view::Target<'a>,
        target: crate::export::Visit<Context<'a>>,
        _: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if matches!(
            metadata.kind,
            crate::export::Kind::Bits | crate::export::Kind::Enum
        ) {
            self.emit(target, writer)?;
        }
        Ok(())
    }

    fn entry(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Self::Error> {
        self.output
            .begin(ctx.entry_id(), ctx.entry_name(), writer)?;
        self.emit(ctx.current(), writer)?;
        writer.write_all(b"endpackage\n")?;
        Ok(())
    }
    fn layout_property(
        &mut self,
        property: LayoutProperty<'_>,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if matches!(
            property.kind(),
            crate::ast::register::LayoutPropertyKind::End
                | crate::ast::register::LayoutPropertyKind::OccupiedEnd
        ) {
            return Ok(());
        }
        let name = property.owner().map_or_else(
            || property.name().to_owned(),
            |owner| format!("{owner}_{}", property.name()),
        );
        self.output.package.declare(&name)?;
        write!(writer, "    function automatic logic [63:0] {name}(")?;
        for (index, parameter) in ctx.parameters().iter().enumerate() {
            let name = format!("p_{parameter}");
            super::super::sv::valid_name(&name)?;
            if index != 0 {
                writer.write_all(b", ")?;
            }
            write!(writer, "input logic [63:0] {name}")?;
        }
        write!(writer, ");\n        return ")?;
        self.emit_all(ctx.children(), writer)?;
        writeln!(writer, ";\n    endfunction")?;
        Ok(())
    }
    fn register_map<'a>(
        &mut self,
        map: Map<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if !map.is_projection() {
            if let Some(alignment) = map.alignment() {
                self.output.package.constant(
                    "ALIGNMENT",
                    "logic [63:0]",
                    format_args!("64'd{alignment}"),
                    writer,
                )?;
            }
            self.emit_all(ctx.children(), writer)?;
        }
        if let Some(extent) = map.extent() {
            self.output.package.constant(
                "EXTENT",
                "logic [63:0]",
                format_args!("64'd{extent}"),
                writer,
            )?;
        }
        if map.is_projection() {
            self.emit_all(ctx.children(), writer)
        } else {
            self.emit_all(ctx.projected_children(), writer)
        }
    }
    fn bank<'a>(
        &mut self,
        _: crate::export::view::Bank<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.instances(), writer)
    }
    fn register<'a>(
        &mut self,
        register: Register<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if let Some(site) = ctx.register_site() {
            let name = register_name(site.segments);
            self.output.package.constant(
                &format!("{name}_OFFSET"),
                "logic [63:0]",
                format_args!("64'd{}", site.offset),
                writer,
            )?;
        }
        let name = register_name(ctx.register_segments());
        if ctx.register_location() == RegisterLocation::Root {
            self.output.package.constant(
                "EXTENT",
                "logic [63:0]",
                format_args!("64'd{}", register.width()),
                writer,
            )?;
        }
        self.output.package.constant(
            &format!("{name}_BYTES"),
            "int unsigned",
            format_args!("32'd{}", register.width()),
            writer,
        )?;
        if ctx.register_location() == RegisterLocation::Root {
            self.output.package.constant(
                "ALIGNMENT",
                "logic [63:0]",
                format_args!("64'd{}", register.alignment()),
                writer,
            )?;
        }
        self.output.package.constant(
            &format!("{name}_ENDIAN_LITTLE"),
            "bit",
            format_args!("1'b{}", u8::from(register.endian() == Endian::Little)),
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_READ"),
            "bit",
            format_args!("1'b{}", u8::from(register.access().readable())),
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_WRITE"),
            "bit",
            format_args!("1'b{}", u8::from(register.access().writable())),
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_RESET"),
            "logic [63:0]",
            format_args!("64'h{:016x}", register.reset_value()),
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_WRITABLE"),
            "logic [63:0]",
            format_args!("64'h{:016x}", register.writable_mask()),
            writer,
        )?;
        self.emit_all(ctx.children(), writer)
    }
}

impl ExpressionSyntax for SvRegisterBackend {
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{name}")
    }
    fn literal(&mut self, value: usize) -> String {
        format!("64'd{value}")
    }
}
