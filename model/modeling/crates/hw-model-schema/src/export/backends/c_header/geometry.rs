use super::{CHeaderBackend, CHeaderExportError, naming};
use crate::ast::analysis::UnitId;
use crate::ast::{ChildLayout, MapLayoutPlan};
use crate::export::expression::{ExpressionSyntax, LayoutProperty, format_expression};

impl ExpressionSyntax for CHeaderBackend {
    fn literal(&mut self, value: usize) -> String {
        format!("UINT64_C({value})")
    }
    fn identifier(&mut self, name: &str) -> String {
        naming::c_identifier(name)
    }
}

impl CHeaderBackend {
    pub(super) fn geometry(&mut self, plan: &MapLayoutPlan) -> Result<(), CHeaderExportError> {
        self.geometry_names = naming::GeometryNames::new(&self.prefix)?;
        self.output.line("typedef struct {")?;
        self.output.line("    uint64_t extent;")?;
        self.geometry_names.members.insert("extent", "extent")?;
        for placement in plan.placements() {
            for property in [
                LayoutProperty::Offset,
                LayoutProperty::Count,
                LayoutProperty::Stride,
            ] {
                let name = format!(
                    "{}_{}",
                    naming::c_identifier(placement.reference.name()),
                    property.name()
                );
                self.geometry_names.members.insert(&name, &name)?;
                self.output.line(format_args!("    uint64_t {name};"))?;
            }
        }
        self.output
            .line(format_args!("}} {}_Geometry;", self.prefix))?;
        let mut parameters = Vec::new();
        for parameter in plan.parameters() {
            let name = format!("p_{}", naming::c_identifier(parameter));
            self.geometry_names.parameters.insert(&name, &name)?;
            parameters.push(format!("uint64_t {name}"));
        }
        let signature = if parameters.is_empty() {
            "void".into()
        } else {
            parameters.join(", ")
        };
        self.output.line(format_args!("static inline {0}_Geometry {0}_GEOMETRY({signature}) {{\n    {0}_Geometry result = {{0}};", self.prefix))?;
        if !parameters.is_empty() {
            self.output.line("    uint64_t cursor = 0;")?;
        }
        for placement in plan.placements() {
            let name = naming::c_identifier(placement.reference.name());
            if let ChildLayout::Call { arguments, .. } = &placement.child {
                let arguments = arguments
                    .iter()
                    .map(|expr| format_expression(expr, None, self))
                    .collect::<Result<Vec<_>, _>>()?;
                let target = self.target_prefix(
                    &UnitId::Definition(placement.child_definition),
                    &placement.child_name.to_ascii_lowercase(),
                );
                self.output.line(format_args!(
                    "    {target}_Geometry child_{name} = {target}_GEOMETRY({});",
                    arguments.join(", ")
                ))?;
            }
            for (property, expression) in [
                (LayoutProperty::Offset, &placement.offset),
                (LayoutProperty::Count, &placement.count),
                (LayoutProperty::Stride, &placement.stride),
            ] {
                let expression = format_expression(expression, Some(placement), self)?;
                self.output.line(format_args!(
                    "    result.{name}_{} = {expression};",
                    property.name()
                ))?;
            }
            if !parameters.is_empty() {
                let end = format_expression(&placement.end, Some(placement), self)?;
                self.output
                    .line(format_args!("    uint64_t end_{name} = {end};"))?;
                let cursor = format_expression(&placement.cursor, Some(placement), self)?;
                self.output.line(format_args!("    cursor = {cursor};"))?;
            }
        }
        let extent = format_expression(plan.extent(), None, self)?;
        self.output
            .line(format_args!("    result.extent = {extent};"))?;
        self.output.line("    return result;\n}")
    }
}
