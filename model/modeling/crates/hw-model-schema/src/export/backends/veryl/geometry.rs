//! Private Veryl geometry declarations generated from checked map formulas.
use super::{VerylBackend, VerylExportError, identifier};
use crate::{
    ast::{ChildLayout, LayoutError, MapLayoutPlan},
    export::expression::{ExpressionSyntax, LayoutProperty, format_expression},
};
use std::fmt::Write;

impl ExpressionSyntax for VerylBackend {
    fn maximum(&mut self, left: &str, right: &str) -> String {
        format!("(if {left} >: {right} ? {left} : {right})")
    }
}
impl From<LayoutError> for VerylExportError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}
impl VerylBackend {
    pub(super) fn geometry(&mut self, plan: &MapLayoutPlan) -> Result<(), VerylExportError> {
        self.package
            .constant("ALIGNMENT", "u64", plan.alignment())?;
        self.package.declare("Geometry")?;
        self.package.declare("geometry")?;
        self.package
            .text
            .push_str("    struct Geometry {\n        extent: u64,\n");
        for placement in plan.placements() {
            let name = placement.reference.name();
            if !identifier(name) {
                return Err(VerylExportError::Name(name.into()));
            }
            for property in [
                LayoutProperty::Offset,
                LayoutProperty::Count,
                LayoutProperty::Stride,
            ] {
                writeln!(
                    self.package.text,
                    "        {name}_{}: u64,",
                    property.name()
                )?;
            }
        }
        self.package.text.push_str("    }\n");
        let mut parameters = Vec::new();
        let mut arguments = Vec::new();
        for name in plan.parameters() {
            if !identifier(&format!("p_{name}")) {
                return Err(VerylExportError::Name((*name).into()));
            }
            parameters.push(format!("p_{name}: input u64"));
            arguments.push(format!("p_{name}"));
        }
        let parameters = parameters.join(", ");
        let arguments = arguments.join(", ");
        write!(
            self.package.text,
            "    function geometry({parameters}) -> Geometry {{\n        var result: Geometry;\n"
        )?;
        if !plan.parameters().is_empty() {
            self.package.text.push_str("        var cursor: u64;\n");
        }
        self.package.text.push_str("        result = '0;\n");
        if !plan.parameters().is_empty() {
            self.package.text.push_str("        cursor = 0;\n");
        }
        for placement in plan.placements() {
            let name = placement.reference.name();
            if let ChildLayout::Call { arguments, .. } = &placement.child {
                let arguments = arguments
                    .iter()
                    .map(|expr| format_expression(expr, None, self))
                    .collect::<Result<Vec<_>, _>>()?;
                let package = self.name(&placement.child_definition.into(), placement.child_name);
                writeln!(
                    self.package.text,
                    "        let child_{name}: {package}::Geometry = {package}::geometry({});",
                    arguments.join(", ")
                )?;
            }
            for (property, expression) in [
                (LayoutProperty::Offset, &placement.offset),
                (LayoutProperty::Count, &placement.count),
                (LayoutProperty::Stride, &placement.stride),
            ] {
                let expression = format_expression(expression, Some(placement), self)?;
                writeln!(
                    self.package.text,
                    "        result.{name}_{} = {expression};",
                    property.name()
                )?;
            }
            if !plan.parameters().is_empty() {
                let end = format_expression(&placement.end, Some(placement), self)?;
                writeln!(self.package.text, "        let end_{name}: u64 = {end};")?;
                let cursor = format_expression(&placement.cursor, Some(placement), self)?;
                writeln!(self.package.text, "        cursor = {cursor};")?;
            }
        }
        let extent = format_expression(plan.extent(), None, self)?;
        writeln!(self.package.text, "        result.extent = {extent};")?;
        self.package
            .text
            .push_str("        return result;\n    }\n");
        for placement in plan.placements() {
            for property in [
                LayoutProperty::Offset,
                LayoutProperty::Count,
                LayoutProperty::Stride,
            ] {
                let name = format!("{}_{}", placement.reference.name(), property.name());
                self.package.declare(&name)?;
                write!(
                    self.package.text,
                    "    function {name}({parameters}) -> u64 {{\n        let shape: Geometry = geometry({arguments});\n        return shape.{name};\n    }}\n"
                )?;
            }
        }
        Ok(())
    }
}
