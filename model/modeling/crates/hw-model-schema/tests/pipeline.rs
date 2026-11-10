//! Typed pipeline composition and occurrence-bound source traversal.
use core::convert::Infallible;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Pass, Pipeline},
    ast::SchemaSet,
    export::{record, value},
    record_schema,
};
record_schema! { pub record Header { endian: Little, fields: { value: u32 } } }
record_schema! { pub record Pair { endian: Little, fields: { left: Header, right: Header } } }
fn schema() -> Result<AnalyzedSchema, hw_model_schema::analysis::AnalysisError> {
    let mut set = SchemaSet::new();
    set.add_record::<Pair>();
    AnalyzedSchema::analyze(set, &Bindings::new())
}
struct Names;
impl<'a> Pass<(&'a AnalyzedSchema, ())> for Names {
    type Output = (&'a AnalyzedSchema, Vec<String>);
    type Error = Infallible;
    fn run(self, (schema, ()): (&'a AnalyzedSchema, ())) -> Result<Self::Output, Self::Error> {
        Ok((
            schema,
            schema.units().map(|unit| unit.name.clone()).collect(),
        ))
    }
}
struct Join;
impl<'a> Pass<(&'a AnalyzedSchema, Vec<String>)> for Join {
    type Output = (&'a AnalyzedSchema, String);
    type Error = Infallible;
    fn run(
        self,
        (schema, names): (&'a AnalyzedSchema, Vec<String>),
    ) -> Result<Self::Output, Infallible> {
        Ok((schema, names.join("/")))
    }
}
struct Text<'a>(&'a str);
impl record::Backend for Text<'_> {
    fn entry(
        &mut self,
        _: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        writer.write_all(self.0.as_bytes())
    }
}
impl value::Backend for Text<'_> {
    type Domain = record::Domain;
    type Error = std::io::Error;
    type Context<'a> = hw_model_schema::export::record::Context<'a>;
}

#[test]
fn chained_pass_data_is_borrowed_by_custom_backend() {
    let schema = schema().unwrap();
    let prepared = Pipeline::new((&schema, ()))
        .run(Names)
        .unwrap()
        .run(Join)
        .unwrap();
    let mut sink = Vec::new();
    prepared.generate(|data| Text(data), &mut sink).unwrap();
    assert_eq!(sink, b"Header/PairHeader/Pair");
}
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Rejected,
}
struct Reject;
impl<'a> Pass<(&'a AnalyzedSchema, Vec<String>)> for Reject {
    type Output = (&'a AnalyzedSchema, String);
    type Error = Failure;
    fn run(self, _: (&'a AnalyzedSchema, Vec<String>)) -> Result<Self::Output, Failure> {
        Err(Failure::Rejected)
    }
}
#[test]
fn failed_pass_preserves_typed_error_without_generation() {
    let schema = schema().unwrap();
    let mut sink = Vec::new();
    let mut constructed = false;
    let result = Pipeline::new((&schema, ()))
        .run(Names)
        .unwrap()
        .run(Reject)
        .map(|prepared| {
            prepared
                .generate(
                    |data| {
                        constructed = true;
                        Text(data)
                    },
                    &mut sink,
                )
                .unwrap();
        });
    assert_eq!(result, Err(Failure::Rejected));
    assert!(!constructed);
    assert!(sink.is_empty());
}
#[test]
fn replacing_schema_discards_prior_data_and_requires_fresh_pass() {
    let original = schema().unwrap();
    let mut set = SchemaSet::new();
    set.add_record::<Header>();
    let replacement = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let reset: Pipeline<(&AnalyzedSchema, ())> = Pipeline::new((&original, ()))
        .run(Names)
        .unwrap()
        .run(Replace(&replacement))
        .unwrap();
    let prepared = reset.run(Names).unwrap().run(Join).unwrap();
    assert_eq!(&prepared.input().1, "Header");
    assert!(core::ptr::eq(
        prepared.input().0,
        core::ptr::from_ref(&replacement)
    ));
}

struct BorrowNames;
impl<'a> Pass<(&'a AnalyzedSchema, ())> for BorrowNames {
    type Output = (&'a AnalyzedSchema, Vec<&'a str>);
    type Error = Infallible;
    fn run(self, (schema, ()): (&'a AnalyzedSchema, ())) -> Result<Self::Output, Infallible> {
        Ok((
            schema,
            schema.units().map(|unit| unit.name.as_str()).collect(),
        ))
    }
}
#[test]
fn pass_output_borrows_schema_metadata_through_generation() {
    let schema = schema().unwrap();
    let prepared = Pipeline::new((&schema, ())).run(BorrowNames).unwrap();
    let mut sink = Vec::new();
    prepared
        .generate(|names| Text(names.first().unwrap()), &mut sink)
        .unwrap();
    assert_eq!(sink, b"HeaderHeader");
    for (name, unit) in prepared.input().1.iter().zip(schema.units()) {
        assert!(core::ptr::eq(*name, unit.name.as_str()));
    }
}
#[test]
fn schema_replacement_discards_borrowed_metadata() {
    let original = schema().unwrap();
    let replacement =
        AnalyzedSchema::opaque("Replacement", hw_model_schema::RecordSpec::new(4, 1, &[])).unwrap();
    let reset: Pipeline<(&AnalyzedSchema, ())> = Pipeline::new((&original, ()))
        .run(BorrowNames)
        .unwrap()
        .run(Replace(&replacement))
        .unwrap();
    let prepared = reset.run(BorrowNames).unwrap();
    assert_eq!(&prepared.input().1, &["Replacement"]);
}

struct Replace<'a>(&'a AnalyzedSchema);
impl<'a, Input> Pass<Input> for Replace<'a> {
    type Output = (&'a AnalyzedSchema, ());
    type Error = Infallible;
    fn run(self, _: Input) -> Result<Self::Output, Infallible> {
        Ok((self.0, ()))
    }
}
