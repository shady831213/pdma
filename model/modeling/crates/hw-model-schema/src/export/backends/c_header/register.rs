use crate::export::backends::c_header::fragment::Fragment;
use crate::traversal::register::{Bank, BankProperty, Context, LayoutProperty, Node};
use std::io::Write;
mod metadata;
pub(super) mod naming;

use super::{CHeaderExportError, CRegisterBackend, naming as symbols};
use crate::analysis::{CompleteMapLayout, RegisterMapView};
use crate::ast::{RegisterInfo, RegisterLocation};
use crate::export::value::Backend as _;
use crate::export::{
    expression::{ExpressionSyntax, format_expression},
    register::Backend,
};
type CResult<T = ()> = Result<T, CHeaderExportError>;
#[derive(Debug, Default)]
pub(super) struct Geometry {
    pub(super) section: GeometrySection,
    names: symbols::Scope,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum GeometrySection {
    #[default]
    Documentation,
    Members,
    Assignments,
}
struct Syntax;
impl ExpressionSyntax for Syntax {
    fn literal(&mut self, value: usize) -> String {
        format!("UINT64_C({value})")
    }
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{}", symbols::c_identifier(name))
    }
}

impl Backend for CRegisterBackend {
    fn root(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> CResult {
        self.prefix = self.names.unit_name(ctx.unit()).1;
        self.output = Fragment::default();
        self.emit(ctx.current(), writer)
    }
    fn map_layout<'a>(
        &mut self,
        _: &'a CompleteMapLayout,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> CResult {
        let mut names = symbols::GeometryNames::new(&self.prefix)?;
        self.geometry = Geometry::default();
        self.emit_all(ctx.children(), writer)?;
        let parameters: Vec<_> = ctx
            .parameters()
            .iter()
            .map(|name| format!("p_{}", symbols::c_identifier(name)))
            .collect();
        Fragment::line("typedef struct {", writer)?;
        Fragment::line("    uint64_t extent;", writer)?;
        self.geometry.section = GeometrySection::Members;
        self.emit_all(ctx.children(), writer)?;
        Fragment::line(format_args!("}} {}_Geometry;", self.prefix), writer)?;
        for parameter in &parameters {
            names.parameters.insert(parameter, parameter)?;
        }
        let signature = if parameters.is_empty() {
            "void".into()
        } else {
            parameters
                .iter()
                .map(|name| format!("uint64_t {name}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        Fragment::line(
            format_args!(
                "static inline {0}_Geometry {0}_GEOMETRY({signature}) {{\n    {0}_Geometry result = {{0}};",
                self.prefix
            ),
            writer,
        )?;
        for parameter in &parameters {
            Fragment::line(format_args!("    (void){parameter};"), writer)?;
        }
        self.geometry.section = GeometrySection::Assignments;
        self.emit_all(ctx.children(), writer)?;
        self.geometry.section = GeometrySection::Documentation;
        Fragment::line("    return result;\n}", writer)
    }
    fn register_map<'a>(
        &mut self,
        value: &'a RegisterMapView,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> CResult {
        self.map_constants(value.extent, writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn bank<'a>(&mut self, _: Bank<'a>, ctx: Context<'a>, writer: &mut dyn Write) -> CResult {
        writer.write_all(b"\n")?;
        self.emit_all(ctx.children(), writer)
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
        if ctx.bank().is_some() {
            let name = self.bank_name(&ctx)?;
            let suffix = if property.name == "offset" {
                "BASE".into()
            } else {
                property.name.to_ascii_uppercase()
            };
            return self.output.unsigned::<64>(
                &format!("{name}_{suffix}"),
                property.expression.evaluate(&[], &[])?,
                writer,
            );
        }
        if self.geometry.section == GeometrySection::Documentation {
            return Ok(());
        }
        let name = if let Some(owner) = property.owner {
            let name = format!("{}_{}", symbols::c_identifier(owner), property.name);
            if self.geometry.section == GeometrySection::Members {
                self.geometry.names.insert(&name, &name)?;
                Fragment::line(format_args!("    uint64_t {name};"), writer)?;
            }
            name
        } else {
            property.name.into()
        };
        if self.geometry.section == GeometrySection::Assignments {
            let expression = format_expression(property.expression, &mut Syntax)?;
            Fragment::line(format_args!("    result.{name} = {expression};"), writer)?;
        }
        Ok(())
    }
    fn register(
        &mut self,
        register: &RegisterInfo,
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
            ctx.children()
                .filter(|visit| matches!(visit.node(), Node::RegisterField(_))),
            writer,
        )?;
        if let Some((name, offset)) = offset {
            self.output.unsigned::<64>(&name, offset, writer)?;
        }
        self.emit_all(
            ctx.children()
                .filter(|visit| matches!(visit.node(), Node::Attribute(_))),
            writer,
        )
    }
    fn register_width(
        &mut self,
        value: usize,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        let name = self.register_name(&ctx)?;
        self.output
            .unsigned::<64>(&format!("{name}_WIDTH"), value, writer)
    }
    fn register_access(
        &mut self,
        value: crate::Access,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        let name = self.register_name(&ctx)?;
        metadata::register_access(&mut self.output, &self.prefix, &name, value, writer)
    }
    fn register_reset(
        &mut self,
        kind: crate::ResetKind,
        value: u64,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        let name = self.register_name(&ctx)?;
        metadata::register_reset(&mut self.output, &self.prefix, &name, kind, value, writer)
    }
    fn register_writable_mask(
        &mut self,
        value: u64,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        let name = self.register_name(&ctx)?;
        self.output
            .unsigned::<64>(&format!("{name}_WRITABLE_MASK"), value, writer)
    }
}
