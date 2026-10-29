use super::{VerylExportError, VerylLayoutExporter, identifier};
use crate::graph::{DefinitionBody, LayoutError, LayoutExpr, MapDefinition};
use std::fmt::Write;

pub(super) fn render(
    text: &mut String,
    map: &MapDefinition,
    exporter: &VerylLayoutExporter,
) -> Result<(), VerylExportError> {
    write!(
        text,
        "    const ALIGNMENT: u64 = {};\n    struct Geometry {{\n        extent: u64,\n",
        map.alignment()
    )?;
    for placement in map.placements() {
        let name = placement.reference().name();
        if !identifier(name) {
            return Err(VerylExportError::Name(name.into()));
        }
        write!(
            text,
            "        {name}_offset: u64,\n        {name}_count: u64,\n        {name}_stride: u64,\n"
        )?;
    }
    text.push_str("    }\n    function geometry(");
    let parameters: Vec<_> = map
        .parameters()
        .iter()
        .map(|name| format!("p_{name}: input u64"))
        .collect();
    text.push_str(&parameters.join(", "));
    text.push_str(") -> Geometry {\n        var result: Geometry;\n        var cursor: u64;\n        result = '0;\n        cursor = 0;\n");
    for placement in map.placements() {
        let name = placement.reference().name();
        let child = placement.reference().target();
        let package = exporter.name(&child);
        let extent = match child.body() {
            DefinitionBody::RegisterMap(_) => {
                let arguments = placement
                    .arguments()
                    .iter()
                    .map(expression)
                    .collect::<Result<Vec<_>, _>>()?;
                writeln!(
                    text,
                    "        let child_{name}: {package}::Geometry = {package}::geometry({});",
                    arguments.join(", ")
                )?;
                format!("child_{name}.extent")
            }
            DefinitionBody::Register(_) | DefinitionBody::Opaque(_) => {
                if !placement.arguments().is_empty() {
                    return Err(LayoutError::Arguments.into());
                }
                format!("{package}::EXTENT")
            }
            DefinitionBody::Bits(_) | DefinitionBody::Record(_) | DefinitionBody::Union { .. } => {
                return Err(LayoutError::Geometry.into());
            }
        };
        let alignment = format!("{package}::ALIGNMENT");
        let offset = placement.offset().map_or_else(
            || format!("(cursor + {alignment} - 1) & ~({alignment} - 1)"),
            |value| value.to_string(),
        );
        let count = expression(placement.count())?;
        write!(
            text,
            "        result.{name}_offset = {offset};\n        result.{name}_count = {count};\n        result.{name}_stride = ({extent} + {alignment} - 1) & ~({alignment} - 1);\n        let end_{name}: u64 = result.{name}_offset + (result.{name}_count - 1) * result.{name}_stride + {extent};\n        cursor = if cursor >: end_{name} ? cursor : end_{name};\n"
        )?;
    }
    let extent = map
        .explicit_extent()
        .map_or_else(|| "cursor".to_owned(), |value| value.to_string());
    write!(
        text,
        "        result.extent = {extent};\n        return result;\n    }}\n"
    )?;
    let signature = parameters.join(", ");
    let arguments = map
        .parameters()
        .iter()
        .map(|name| format!("p_{name}"))
        .collect::<Vec<_>>()
        .join(", ");
    for placement in map.placements() {
        for property in ["offset", "count", "stride"] {
            let name = format!("{}_{property}", placement.reference().name());
            write!(
                text,
                "    function {name}({signature}) -> u64 {{\n        let shape: Geometry = geometry({arguments});\n        return shape.{name};\n    }}\n"
            )?;
        }
    }
    Ok(())
}

fn expression(expr: &LayoutExpr) -> Result<String, VerylExportError> {
    match expr {
        LayoutExpr::Constant(value) => Ok(value.to_string()),
        LayoutExpr::Parameter(name) => Ok(format!("p_{name}")),
        LayoutExpr::Add(a, b) => Ok(format!("({} + {})", expression(a)?, expression(b)?)),
        LayoutExpr::Subtract(a, b) => Ok(format!("({} - {})", expression(a)?, expression(b)?)),
        LayoutExpr::Multiply(a, b) => Ok(format!("({} * {})", expression(a)?, expression(b)?)),
        LayoutExpr::Unsupported(source) => Err(LayoutError::Unsupported(source).into()),
    }
}
