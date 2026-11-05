use crate::traversal::register::{Bank, BankProperty, Context, LayoutProperty, Node};
mod metadata;
pub(super) mod naming;

use super::{CHeaderExportError, CRegisterBackend, fragment::Fragment, naming as symbols};
use crate::analysis::{CompleteMapLayout, RegisterMapView};
use crate::ast::{LayoutExpr, RegisterInfo, RegisterLocation};
use crate::export::value::Backend as _;
use crate::export::{
    Artifact,
    expression::{ExpressionSyntax, format_expression},
    register::Backend,
};
type CResult<T = ()> = Result<T, CHeaderExportError>;
#[derive(Debug, Default)]
pub(super) struct Geometry {
    members: Fragment,
    assignments: Fragment,
    names: symbols::Scope,
}

impl Geometry {
    fn assignment(
        &mut self,
        name: &str,
        expression: &LayoutExpr,
    ) -> Result<(), CHeaderExportError> {
        let expression = format_expression(expression, &mut Syntax)?;
        self.assignments
            .line(format_args!("    result.{name} = {expression};"))
    }
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
    fn root(&mut self, ctx: Context<'_>) -> CResult {
        (self.logical_name, self.prefix) = self.names.unit(ctx.unit())?;
        self.output = Fragment::default();
        self.emit(ctx.current())?;
        self.artifacts.push(Artifact {
            logical_name: self.logical_name.clone(),
            media_type: "text/x-c; charset=utf-8".into(),
            contents: std::mem::take(&mut self.output.text).into_bytes(),
        });
        Ok(())
    }
    fn map_layout<'a>(&mut self, _: &'a CompleteMapLayout, ctx: Context<'a>) -> CResult {
        let mut names = symbols::GeometryNames::new(&self.prefix)?;
        self.geometry = Geometry::default();
        self.emit_all(ctx.children())?;
        let parameters: Vec<_> = ctx
            .parameters()
            .iter()
            .map(|name| format!("p_{}", symbols::c_identifier(name)))
            .collect();
        self.output.line("typedef struct {")?;
        self.output.line("    uint64_t extent;")?;
        self.output.text.push_str(&self.geometry.members.text);
        self.output
            .line(format_args!("}} {}_Geometry;", self.prefix))?;
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
        self.output.line(format_args!("static inline {0}_Geometry {0}_GEOMETRY({signature}) {{\n    {0}_Geometry result = {{0}};", self.prefix))?;
        for parameter in &parameters {
            self.output.line(format_args!("    (void){parameter};"))?;
        }
        self.output.text.push_str(&self.geometry.assignments.text);
        self.output.line("    return result;\n}")
    }
    fn register_map<'a>(&mut self, value: &'a RegisterMapView, ctx: Context<'a>) -> CResult {
        self.map_constants(value.extent)?;
        self.emit_all(ctx.children())
    }
    fn bank<'a>(&mut self, _: Bank<'a>, ctx: Context<'a>) -> CResult {
        self.output.text.push('\n');
        self.emit_all(ctx.children())
    }
    fn bank_property(&mut self, property: BankProperty, ctx: Context<'_>) -> CResult {
        let name = self.bank_name(&ctx)?;
        let (suffix, value) = match property {
            BankProperty::Index(v) => ("INDEX", v),
            BankProperty::ParentIndex(v) => ("PARENT_INDEX", v),
            BankProperty::ParentInstance(v) => ("PARENT_INSTANCE", v),
        };
        self.output
            .unsigned::<64>(&format!("{name}_{suffix}"), value)
    }
    fn layout_property(&mut self, property: LayoutProperty<'_>, ctx: Context<'_>) -> CResult {
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
            );
        }
        let name = if let Some(owner) = property.owner {
            let name = format!("{}_{}", symbols::c_identifier(owner), property.name);
            self.geometry.names.insert(&name, &name)?;
            self.geometry
                .members
                .line(format_args!("    uint64_t {name};"))?;
            name
        } else {
            property.name.into()
        };
        self.geometry.assignment(&name, property.expression)
    }
    fn register(&mut self, register: &RegisterInfo, ctx: Context<'_>) -> CResult {
        let location = ctx.register_location();
        match location {
            RegisterLocation::Root => {
                self.map_constants(register.width())?;
            }
            RegisterLocation::Standalone(_) | RegisterLocation::Template { .. } => {
                if matches!(location, RegisterLocation::Standalone(_)) {
                    self.output.text.push('\n');
                }
            }
            RegisterLocation::Instance { .. } => {}
        }
        let offset = self.register_offset(&ctx)?;
        if matches!(location, RegisterLocation::Instance { .. }) {
            let (name, offset) =
                offset.ok_or(CHeaderExportError::InvalidVisit("instance offset"))?;
            return self.output.unsigned::<64>(&name, offset);
        }
        self.emit_all(
            ctx.children()
                .filter(|visit| matches!(visit.node(), Node::RegisterField(_))),
        )?;
        if let Some((name, offset)) = offset {
            self.output.unsigned::<64>(&name, offset)?;
        }
        self.emit_all(
            ctx.children()
                .filter(|visit| matches!(visit.node(), Node::Attribute(_))),
        )
    }
    fn register_width(&mut self, value: usize, ctx: Context<'_>) -> CResult {
        let name = self.register_name(&ctx)?;
        self.output.unsigned::<64>(&format!("{name}_WIDTH"), value)
    }
    fn register_access(&mut self, value: crate::Access, ctx: Context<'_>) -> CResult {
        let name = self.register_name(&ctx)?;
        metadata::register_access(&mut self.output, &self.prefix, &name, value)
    }
    fn register_reset(&mut self, kind: crate::ResetKind, value: u64, ctx: Context<'_>) -> CResult {
        let name = self.register_name(&ctx)?;
        metadata::register_reset(&mut self.output, &self.prefix, &name, kind, value)
    }
    fn register_writable_mask(&mut self, value: u64, ctx: Context<'_>) -> CResult {
        let name = self.register_name(&ctx)?;
        self.output
            .unsigned::<64>(&format!("{name}_WRITABLE_MASK"), value)
    }
}
