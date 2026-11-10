use hw_model_schema::export::{self, Visit, record, value};
struct Target;
impl record::Backend for Target {}
impl value::Backend for Target {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = record::Context<'a>;

    fn emit(&mut self, _: Visit<Self::Context<'_>>, _: &mut dyn std::io::Write) -> Result<(), Self::Error> {
        Ok(())
    }
    fn emit_all<'a, I>(&mut self, _: I, _: &mut dyn std::io::Write) -> Result<(), Self::Error>
    where I: IntoIterator<Item = Visit<Self::Context<'a>>> {
        Ok(())
    }
}


impl value::Emit for Target {}
struct CustomDomain;
impl export::Domain<Target> for CustomDomain {
    fn emit(_: Visit<record::Context<'_>>, _: &mut Target, _: &mut dyn std::io::Write) -> Result<(), std::io::Error> { Ok(()) }
    fn reference<'a>(_: &'a hw_model_schema::ast::Reference, _: record::Context<'a>, _: &mut Target, _: &mut dyn std::io::Write) -> Result<(), std::io::Error> { Ok(()) }
    fn validate(_: &hw_model_schema::analysis::Unit) -> Result<(), hw_model_schema::analysis::AnalysisError> { Ok(()) }
    fn entries(_: &hw_model_schema::analysis::AnalyzedSchema) -> impl Iterator<Item = Visit<record::Context<'_>>> { std::iter::empty() }
    fn entry(_: Visit<record::Context<'_>>, _: &mut Target, _: &mut dyn std::io::Write) -> Result<(), std::io::Error> { Ok(()) }
}
fn main() {}
