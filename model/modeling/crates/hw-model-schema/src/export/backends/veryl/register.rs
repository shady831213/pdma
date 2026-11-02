use super::{VerylExportError, VerylRegisterBackend, package::register_name};
use crate::{
    BitsFieldSpec, Endian,
    analysis::RegisterMapView,
    ast::{RegisterInfo, RegisterLocation},
    export::{
        expression::{ExpressionSyntax, format_expression},
        register::{Backend, EmitContext, LayoutProperty},
        value::Backend as _,
    },
};
use std::fmt::Write;

impl Backend for VerylRegisterBackend {
    fn root(&mut self, ctx: EmitContext<'_>) -> Result<(), Self::Error> {
        let name = self.output.begin(ctx.unit())?;
        self.emit(ctx.current())?;
        self.output.end(&name);
        Ok(())
    }
    fn layout_property(
        &mut self,
        property: LayoutProperty<'_>,
        ctx: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
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
            self.output.package.text,
            "    function {name}({}) -> u64 {{\n        return {};\n    }}",
            parameters.join(", "),
            format_expression(property.expression, &mut Syntax)?
        )?;
        Ok(())
    }
    fn register_map<'a>(
        &mut self,
        map: &'a RegisterMapView,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        self.output.package.constant("EXTENT", "u64", map.extent)?;
        self.emit_all(ctx.children())
    }
    fn register<'a>(
        &mut self,
        _: &'a RegisterInfo,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        if let Some(site) = ctx.register_site() {
            if matches!(site.location, RegisterLocation::Template { .. }) {
                return Ok(());
            }
            let name = register_name(site.segments);
            self.output
                .package
                .constant(&format!("{name}_OFFSET"), "u64", site.offset)?;
        }
        self.emit_all(ctx.children())
    }
    fn register_field<'a>(
        &mut self,
        field: &'a BitsFieldSpec<usize>,
        ctx: EmitContext<'a>,
    ) -> Result<(), Self::Error> {
        let prefix = register_name(ctx.register_segments());
        let name = format!("{prefix}_{}", field.name().to_ascii_uppercase());
        self.output
            .package
            .constant(&format!("{name}_LSB"), "u32", field.lsb())?;
        self.output
            .package
            .constant(&format!("{name}_BITS"), "u32", field.bit_width())?;
        self.emit_all(ctx.children())
    }
    fn register_width(&mut self, width: usize, ctx: EmitContext<'_>) -> Result<(), Self::Error> {
        if ctx.register_location() == RegisterLocation::Root {
            self.output.package.constant("EXTENT", "u64", width)?;
        }
        let name = register_name(ctx.register_segments());
        self.output
            .package
            .constant(&format!("{name}_BYTES"), "u32", width)
    }
    fn register_endian(&mut self, endian: Endian, ctx: EmitContext<'_>) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output.package.constant(
            &format!("{name}_ENDIAN_LITTLE"),
            "bbool",
            endian == Endian::Little,
        )
    }
    fn register_reset(
        &mut self,
        _: crate::ResetKind,
        value: u64,
        ctx: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output.package.constant(
            &format!("{name}_RESET"),
            "u64",
            format_args!("64'h{value:016x}"),
        )
    }
    fn register_writable_mask(
        &mut self,
        value: u64,
        ctx: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output.package.constant(
            &format!("{name}_WRITABLE"),
            "u64",
            format_args!("64'h{value:016x}"),
        )
    }
    fn register_access(
        &mut self,
        access: crate::Access,
        ctx: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        let name = register_name(ctx.register_segments());
        self.output
            .package
            .constant(&format!("{name}_READ"), "bbool", access.readable())?;
        self.output
            .package
            .constant(&format!("{name}_WRITE"), "bbool", access.writable())
    }
    fn register_alignment(
        &mut self,
        value: usize,
        ctx: EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        if ctx.register_location() == RegisterLocation::Root {
            self.output.package.constant("ALIGNMENT", "u64", value)?;
        }
        Ok(())
    }
    fn map_alignment(&mut self, value: usize, _: EmitContext<'_>) -> Result<(), Self::Error> {
        self.output.package.constant("ALIGNMENT", "u64", value)
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
