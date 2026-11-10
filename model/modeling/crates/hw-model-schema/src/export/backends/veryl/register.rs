use super::{VerylRegisterBackend, package::register_name};
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
use std::io::Write;

impl Backend for VerylRegisterBackend {
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
        writer.write_all(b"}\n")?;
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
        write!(writer, "    function {name}(")?;
        for (index, parameter) in ctx.parameters().iter().enumerate() {
            let name = format!("p_{parameter}");
            if !super::identifier::is_identifier(&name) {
                return Err(super::VerylExportError::Name(name));
            }
            if index != 0 {
                writer.write_all(b", ")?;
            }
            write!(writer, "{name}: input u64")?;
        }
        write!(writer, ") -> u64 {{\n        return ")?;
        self.emit_all(ctx.children(), writer)?;
        writeln!(writer, ";\n    }}")?;
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
                self.output
                    .package
                    .constant("ALIGNMENT", "u64", alignment, writer)?;
            }
            self.emit_all(ctx.children(), writer)?;
        }
        if let Some(extent) = map.extent() {
            self.output
                .package
                .constant("EXTENT", "u64", extent, writer)?;
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
            self.output
                .package
                .constant(&format!("{name}_OFFSET"), "u64", site.offset, writer)?;
        }
        let name = register_name(ctx.register_segments());
        if ctx.register_location() == RegisterLocation::Root {
            self.output
                .package
                .constant("EXTENT", "u64", register.width(), writer)?;
        }
        self.output
            .package
            .constant(&format!("{name}_BYTES"), "u32", register.width(), writer)?;
        if ctx.register_location() == RegisterLocation::Root {
            self.output
                .package
                .constant("ALIGNMENT", "u64", register.alignment(), writer)?;
        }
        self.output.package.constant(
            &format!("{name}_ENDIAN_LITTLE"),
            "bbool",
            register.endian() == Endian::Little,
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_READ"),
            "bbool",
            register.access().readable(),
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_WRITE"),
            "bbool",
            register.access().writable(),
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_RESET"),
            "u64",
            format_args!("64'h{:016x}", register.reset_value()),
            writer,
        )?;
        self.output.package.constant(
            &format!("{name}_WRITABLE"),
            "u64",
            format_args!("64'h{:016x}", register.writable_mask()),
            writer,
        )?;
        self.emit_all(ctx.children(), writer)
    }
}

impl ExpressionSyntax for VerylRegisterBackend {
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{name}")
    }
    fn maximum(&mut self, left: &str, right: &str) -> String {
        format!("(if {left} >: {right} ? {left} : {right})")
    }
}
