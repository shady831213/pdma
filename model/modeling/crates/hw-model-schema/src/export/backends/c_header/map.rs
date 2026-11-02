use super::{CHeaderBackend, CHeaderExportError, naming};
use crate::ast::analysis::{
    UnitId,
    program::{Call, LayoutProgram, Member, Symbol},
};
use crate::export::{
    expression::ExpressionSyntax,
    program::{ProgramSyntax, emit_body},
};

impl ExpressionSyntax for CHeaderBackend {
    fn literal(&mut self, value: usize) -> String {
        format!("UINT64_C({value})")
    }
    fn identifier(&mut self, name: &str) -> String {
        naming::c_identifier(name)
    }
}

impl ProgramSyntax for CHeaderBackend {
    type Error = CHeaderExportError;

    fn call(&mut self, call: &Call, arguments: &[String]) -> Result<(), Self::Error> {
        let name = naming::c_identifier(call.name);
        let target = self.target_prefix(
            &UnitId::Definition(call.definition),
            &call.definition_name.to_ascii_lowercase(),
        );
        self.output.line(format_args!(
            "    {target}_Geometry child_{name} = {target}_GEOMETRY({});",
            arguments.join(", ")
        ))
    }

    fn define(&mut self, target: Symbol, value: &str) -> Result<(), Self::Error> {
        let target = self.reference(target);
        self.output
            .line(format_args!("    uint64_t {target} = {value};"))
    }

    fn assign(&mut self, target: Symbol, value: &str) -> Result<(), Self::Error> {
        let target = self.reference(target);
        self.output.line(format_args!("    {target} = {value};"))
    }
}

impl CHeaderBackend {
    pub(super) fn map_layout(&mut self, program: &LayoutProgram) -> Result<(), CHeaderExportError> {
        let mut names = naming::GeometryNames::new(&self.prefix)?;
        self.output.line("typedef struct {")?;
        for member in &program.members {
            let name = match member {
                Member::Extent => "extent".into(),
                Member::Placement { name, property } => {
                    format!("{}_{}", naming::c_identifier(name), property.name())
                }
            };
            names.members.insert(&name, &name)?;
            self.output.line(format_args!("    uint64_t {name};"))?;
        }
        self.output
            .line(format_args!("}} {}_Geometry;", self.prefix))?;
        let mut parameters = Vec::new();
        for parameter in &program.parameters {
            let name = self.reference(Symbol::Parameter(parameter));
            names.parameters.insert(&name, &name)?;
            parameters.push(format!("uint64_t {name}"));
        }
        let signature = if parameters.is_empty() {
            "void".into()
        } else {
            parameters.join(", ")
        };
        self.output.line(format_args!("static inline {0}_Geometry {0}_GEOMETRY({signature}) {{\n    {0}_Geometry result = {{0}};", self.prefix))?;
        for local in &program.locals {
            let name = self.reference(*local);
            self.output.line(format_args!("    uint64_t {name};"))?;
        }
        emit_body(program, self)?;
        self.output.line("    return result;\n}")
    }
}
