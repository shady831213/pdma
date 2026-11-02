use super::{VerylBackend, VerylExportError, identifier};
use crate::{
    ast::analysis::program::{Call, LayoutProgram, Member, Symbol},
    export::{
        expression::ExpressionSyntax,
        program::{ProgramSyntax, emit_body},
    },
};
use std::fmt::Write;

impl ExpressionSyntax for VerylBackend {
    fn maximum(&mut self, left: &str, right: &str) -> String {
        format!("(if {left} >: {right} ? {left} : {right})")
    }
}

impl ProgramSyntax for VerylBackend {
    type Error = VerylExportError;

    fn call(&mut self, call: &Call, arguments: &[String]) -> Result<(), Self::Error> {
        let name = call.name;
        let package = self.name(&call.definition.into(), call.definition_name);
        writeln!(
            self.package.text,
            "        let child_{name}: {package}::Geometry = {package}::geometry({});",
            arguments.join(", ")
        )?;
        Ok(())
    }

    fn define(&mut self, target: Symbol, value: &str) -> Result<(), Self::Error> {
        let target = self.reference(target);
        writeln!(self.package.text, "        let {target}: u64 = {value};")?;
        Ok(())
    }

    fn assign(&mut self, target: Symbol, value: &str) -> Result<(), Self::Error> {
        let target = self.reference(target);
        writeln!(self.package.text, "        {target} = {value};")?;
        Ok(())
    }
}

impl VerylBackend {
    pub(super) fn map_layout(&mut self, program: &LayoutProgram) -> Result<(), VerylExportError> {
        self.package
            .constant("ALIGNMENT", "u64", program.alignment)?;
        self.package.declare("Geometry")?;
        self.package.declare("geometry")?;
        self.package.text.push_str("    struct Geometry {\n");
        for member in &program.members {
            let name = match member {
                Member::Extent => "extent".into(),
                Member::Placement { name, property } => {
                    if !identifier(name) {
                        return Err(VerylExportError::Name((*name).into()));
                    }
                    format!("{name}_{}", property.name())
                }
            };
            writeln!(self.package.text, "        {name}: u64,")?;
        }
        self.package.text.push_str("    }\n");
        let mut parameters = Vec::new();
        let mut arguments = Vec::new();
        for name in &program.parameters {
            let parameter = self.reference(Symbol::Parameter(name));
            if !identifier(&parameter) {
                return Err(VerylExportError::Name((*name).into()));
            }
            parameters.push(format!("{parameter}: input u64"));
            arguments.push(parameter);
        }
        let parameters = parameters.join(", ");
        let arguments = arguments.join(", ");
        write!(
            self.package.text,
            "    function geometry({parameters}) -> Geometry {{\n        var result: Geometry;\n"
        )?;
        for local in &program.locals {
            let name = self.reference(*local);
            writeln!(self.package.text, "        var {name}: u64;")?;
        }
        self.package.text.push_str("        result = '0;\n");
        emit_body(program, self)?;
        self.package
            .text
            .push_str("        return result;\n    }\n");
        for member in &program.members {
            match member {
                Member::Extent => {}
                Member::Placement { name, property } => {
                    let name = format!("{name}_{}", property.name());
                    self.package.declare(&name)?;
                    write!(
                        self.package.text,
                        "    function {name}({parameters}) -> u64 {{\n        let shape: Geometry = geometry({arguments});\n        return shape.{name};\n    }}\n"
                    )?;
                }
            }
        }
        Ok(())
    }
}
