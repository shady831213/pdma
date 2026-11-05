use super::{VerylExportError, VerylRegisterBackend, package::register_name};
use crate::traversal::register::{Context, LayoutProperty};
use crate::{
    BitsFieldSpec, Endian,
    analysis::RegisterMapView,
    ast::{RegisterInfo, RegisterLocation},
    export::{
        expression::{ExpressionSyntax, format_expression},
        register::Backend,
        value::Backend as _,
    },
};
use std::io::Write;

impl Backend for VerylRegisterBackend {
    fn root(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Self::Error> {
        self.output.begin(ctx.unit(), writer)?;
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
        if ctx.bank().is_some() {
            return Ok(());
        }
        let name = property.owner.map_or_else(
            || property.name.to_owned(),
            |owner| format!("{owner}_{}", property.name),
        );
        let parameters = ctx
            .parameters()
            .iter()
            .map(|name| {
                let name = format!("p_{name}");
                if !super::identifier(&name) {
                    return Err(VerylExportError::Name(name));
                }
                Ok(format!("{name}: input u64"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.output.package.declare(&name)?;
        writeln!(
            writer,
            "    function {name}({}) -> u64 {{\n        return {};\n    }}",
            parameters.join(", "),
            format_expression(property.expression, &mut Syntax)?
        )?;
        Ok(())
    }
    fn register_map<'a>(
        &mut self,
        map: &'a RegisterMapView,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.output
            .package
            .constant("EXTENT", "u64", map.extent, writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn register<'a>(
        &mut self,
        _: &'a RegisterInfo,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if let Some(site) = ctx.register_site() {
            if matches!(site.location, RegisterLocation::Template { .. }) {
                return Ok(());
            }
            let name = register_name(site.segments);
            self.output
                .package
                .constant(&format!("{name}_OFFSET"), "u64", site.offset, writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn register_field<'a>(
        &mut self,
        field: &'a BitsFieldSpec<usize>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let prefix = register_name(ctx.register_segments());
        let name = format!("{prefix}_{}", field.name().to_ascii_uppercase());
        self.output
            .package
            .constant(&format!("{name}_LSB"), "u32", field.lsb(), writer)?;
        self.output
            .package
            .constant(&format!("{name}_BITS"), "u32", field.bit_width(), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn register_width(
        &mut self,
        width: usize,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.register_location() == RegisterLocation::Root {
            self.output
                .package
                .constant("EXTENT", "u64", width, writer)?;
        }
        let name = register_name(ctx.register_segments());
        self.output
            .package
            .constant(&format!("{name}_BYTES"), "u32", width, writer)
    }
    fn register_endian(
        &mut self,
        endian: Endian,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output.package.constant(
            &format!("{name}_ENDIAN_LITTLE"),
            "bbool",
            endian == Endian::Little,
            writer,
        )
    }
    fn register_reset(
        &mut self,
        _: crate::ResetKind,
        value: u64,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output.package.constant(
            &format!("{name}_RESET"),
            "u64",
            format_args!("64'h{value:016x}"),
            writer,
        )
    }
    fn register_writable_mask(
        &mut self,
        value: u64,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output.package.constant(
            &format!("{name}_WRITABLE"),
            "u64",
            format_args!("64'h{value:016x}"),
            writer,
        )
    }
    fn register_access(
        &mut self,
        access: crate::Access,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output.package.constant(
            &format!("{name}_READ"),
            "bbool",
            access.readable(),
            writer,
        )?;
        self.output
            .package
            .constant(&format!("{name}_WRITE"), "bbool", access.writable(), writer)
    }
    fn register_alignment(
        &mut self,
        value: usize,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.register_location() == RegisterLocation::Root {
            self.output
                .package
                .constant("ALIGNMENT", "u64", value, writer)?;
        }
        Ok(())
    }
    fn map_alignment(
        &mut self,
        value: usize,
        _: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.output
            .package
            .constant("ALIGNMENT", "u64", value, writer)
    }
}

struct Syntax;
impl ExpressionSyntax for Syntax {
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{name}")
    }
    fn maximum(&mut self, left: &str, right: &str) -> String {
        format!("(if {left} >: {right} ? {left} : {right})")
    }
}
