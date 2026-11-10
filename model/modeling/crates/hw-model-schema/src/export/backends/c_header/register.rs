use crate::export::Kind;
use crate::export::backends::c_header::fragment::Fragment;
use crate::export::register::{Bank, BankProperty, Context, LayoutProperty};
use std::io::Write;
pub(super) mod geometry;
mod metadata;
pub(super) mod naming;

use super::{CHeaderExportError, CRegisterBackend, naming as symbols};
use crate::ast::RegisterLocation;
use crate::export::value::Emit as _;
use crate::export::view::{Map, Register};
use crate::export::{expression::ExpressionSyntax, register::Backend};
type CResult<T = ()> = Result<T, CHeaderExportError>;
impl ExpressionSyntax for CRegisterBackend {
    fn literal(&mut self, value: usize) -> String {
        format!("UINT64_C({value})")
    }
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{}", symbols::c_identifier(name))
    }
}

impl Backend for CRegisterBackend {
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

    fn entry(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> CResult {
        self.prefix = self.names.entry_prefix(ctx.entry_id(), ctx.entry_name());
        self.output = Fragment::default();
        self.emit(ctx.current(), writer)
    }
    fn register_map<'a>(
        &mut self,
        map: Map<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> CResult {
        if map.is_projection() {
            self.map_constants(
                map.extent()
                    .ok_or(CHeaderExportError::InvalidVisit("concrete map extent"))?,
                writer,
            )?;
            self.emit_all(ctx.children(), writer)
        } else {
            self.emit_geometry(&ctx, writer)?;
            if let Some(extent) = map.extent() {
                self.map_constants(extent, writer)?;
                self.emit_all(ctx.projected_children(), writer)?;
            }
            Ok(())
        }
    }
    fn bank<'a>(&mut self, _: Bank<'a>, ctx: Context<'a>, writer: &mut dyn Write) -> CResult {
        writer.write_all(b"\n")?;
        self.emit_all(ctx.children(), writer)?;
        self.emit_all(ctx.instances(), writer)
    }
    fn bank_property(
        &mut self,
        property: BankProperty,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        let name = self.bank_name(&ctx)?;
        let (suffix, value) = match property {
            BankProperty::Index(v) => ("INDEX", v),
            BankProperty::ParentIndex(v) => ("PARENT_INDEX", v),
            BankProperty::ParentInstance(v) => ("PARENT_INSTANCE", v),
        };
        self.output
            .unsigned::<64>(&format!("{name}_{suffix}"), value, writer)
    }
    fn layout_property(
        &mut self,
        property: LayoutProperty<'_>,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if matches!(
            property.kind(),
            crate::ast::register::LayoutPropertyKind::End
                | crate::ast::register::LayoutPropertyKind::OccupiedEnd
        ) {
            return Ok(());
        }
        if ctx.bank().is_some() {
            let name = self.bank_name(&ctx)?;
            let suffix = if property.name() == "offset" {
                "BASE".into()
            } else {
                property.name().to_ascii_uppercase()
            };
            let name = format!("{name}_{suffix}");
            self.output.declare(&name)?;
            write!(writer, "#define {name} UINT64_C(")?;
            self.emit_all(ctx.children(), writer)?;
            writeln!(writer, ")")?;
            return Ok(());
        }
        self.geometry_property(property, &ctx)
    }
    fn register(
        &mut self,
        register: Register<'_>,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        let location = ctx.register_location();
        match location {
            RegisterLocation::Root => {
                self.map_constants(register.width(), writer)?;
            }
            RegisterLocation::Standalone(_) | RegisterLocation::Template { .. } => {
                if matches!(location, RegisterLocation::Standalone(_)) {
                    writer.write_all(b"\n")?;
                }
            }
            RegisterLocation::Instance { .. } => {}
        }
        let offset = self.register_offset(&ctx)?;
        if matches!(location, RegisterLocation::Instance { .. }) {
            let (name, offset) =
                offset.ok_or(CHeaderExportError::InvalidVisit("instance offset"))?;
            return self.output.unsigned::<64>(&name, offset, writer);
        }
        self.emit_all(
            ctx.children().filter(|visit| {
                matches!(
                    visit.kind(),
                    Kind::RegisterField | Kind::Bits | Kind::Reference
                )
            }),
            writer,
        )?;
        if let Some((name, offset)) = offset {
            self.output.unsigned::<64>(&name, offset, writer)?;
        }
        let name = self.register_name(&ctx)?;
        self.output
            .unsigned::<64>(&format!("{name}_WIDTH"), register.width(), writer)?;
        metadata::register_access(
            &mut self.output,
            &self.prefix,
            &name,
            register.access(),
            writer,
        )?;
        metadata::register_reset(
            &mut self.output,
            &self.prefix,
            &name,
            register.reset_kind(),
            register.reset_value(),
            writer,
        )?;
        self.output.unsigned::<64>(
            &format!("{name}_WRITABLE_MASK"),
            register.writable_mask(),
            writer,
        )
    }
}
