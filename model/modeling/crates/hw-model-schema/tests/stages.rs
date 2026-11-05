//! Public typed stages preserve validation and invalidate derived geometry.
use core::convert::Infallible;
use hw_model_schema::{
    RecordSpec,
    analysis::{
        AnalyzedSchema, Bindings, Layout, LayoutSchema, Pass, Pipeline, Resolve, Simplify, Validate,
    },
    ast::{Definition, SchemaGraph, SchemaSet},
};
struct Storage;
fn source(size: usize) -> SchemaSet {
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<Storage>(RecordSpec::new(size, 1, &[])));
    set
}
struct Resize(usize);
impl Pass<&SchemaGraph> for Resize {
    type Output = SchemaGraph;
    type Error = hw_model_schema::analysis::AnalysisError;
    fn run(self, input: &SchemaGraph) -> Result<SchemaGraph, Self::Error> {
        let definition = input
            .get(hw_model_schema::ast::DefinitionId::of::<Storage>())
            .ok_or(hw_model_schema::ast::LayoutError::Geometry)?;
        let hw_model_schema::ast::DefinitionBody::Opaque(spec) = definition.body() else {
            return Err(hw_model_schema::ast::LayoutError::Geometry.into());
        };
        Resolve.run(source(
            spec.size()
                .checked_add(self.0)
                .ok_or(hw_model_schema::ast::LayoutError::Geometry)?,
        ))
    }
}
struct Inspect<'a>(&'a mut Vec<&'static str>, &'static str);
impl<T> Pass<T> for Inspect<'_> {
    type Output = T;
    type Error = Infallible;
    fn run(self, input: T) -> Result<T, Infallible> {
        self.0.push(self.1);
        Ok(input)
    }
}
fn extent(schema: &AnalyzedSchema) -> Option<usize> {
    match &schema.units().next()?.body {
        Layout::Record(record) => Some(record.size()),
        Layout::None | Layout::Map(_) => None,
    }
}
#[test]
fn custom_passes_run_at_each_stage_and_transform_before_layout() {
    let mut order = Vec::new();
    let graph = Pipeline::new(source(4))
        .run(Inspect(&mut order, "source"))
        .unwrap()
        .run(Resolve)
        .unwrap()
        .run(Inspect(&mut order, "resolved"))
        .unwrap()
        .into_inner();
    let graph = Pipeline::new(&graph).run(Resize(12)).unwrap().into_inner();
    let bindings = Bindings::new();
    let analyzed = Pipeline::new(&graph)
        .run(Validate::new(&bindings))
        .unwrap()
        .run(Inspect(&mut order, "validated"))
        .unwrap()
        .run(LayoutSchema)
        .unwrap()
        .run(Inspect(&mut order, "layout"))
        .unwrap()
        .run(Simplify)
        .unwrap()
        .run(Inspect(&mut order, "simplified"))
        .unwrap()
        .into_inner();
    assert_eq!(extent(&analyzed), Some(16));
    assert_eq!(
        order,
        ["source", "resolved", "validated", "layout", "simplified"]
    );
    assert_eq!(
        extent(&AnalyzedSchema::analyze(source(16), &bindings).unwrap()),
        extent(&analyzed)
    );
}
struct BorrowNames;
impl<'a> Pass<&'a AnalyzedSchema> for BorrowNames {
    type Output = Vec<&'a str>;
    type Error = Infallible;
    fn run(self, input: &'a AnalyzedSchema) -> Result<Self::Output, Infallible> {
        Ok(input.units().map(|unit| unit.name.as_str()).collect())
    }
}
impl<'a> Pass<&'a hw_model_schema::analysis::LaidOutSchema<'_>> for BorrowNames {
    type Output = Vec<&'a str>;
    type Error = Infallible;
    fn run(
        self,
        input: &'a hw_model_schema::analysis::LaidOutSchema<'_>,
    ) -> Result<Self::Output, Infallible> {
        Ok(input.units().map(|unit| unit.name.as_str()).collect())
    }
}
#[test]
fn read_only_pass_borrows_post_layout_metadata() {
    let graph = Resolve.run(source(8)).unwrap();
    let bindings = Bindings::new();
    let laid = LayoutSchema
        .run(Validate::new(&bindings).run(&graph).unwrap())
        .unwrap();
    let names = Pipeline::new(&laid).run(BorrowNames).unwrap().into_inner();
    assert!(core::ptr::eq(
        *names.first().unwrap(),
        laid.units().next().unwrap().name.as_str()
    ));

    let schema = AnalyzedSchema::analyze(source(8), &Bindings::new()).unwrap();
    let names = Pipeline::new(&schema)
        .run(BorrowNames)
        .unwrap()
        .into_inner();
    assert!(core::ptr::eq(
        *names.first().unwrap(),
        schema.units().next().unwrap().name.as_str()
    ));
}
#[test]
fn returning_to_source_discards_layout_and_recomputes() {
    let bindings = Bindings::new();
    let graph = Resolve.run(source(4)).unwrap();
    let laid = LayoutSchema
        .run(Validate::new(&bindings).run(&graph).unwrap())
        .unwrap();
    let old: &SchemaGraph = laid.into_source();
    assert_eq!(old.definitions().count(), 1);
    let replacement = Resize(28).run(old).unwrap();
    let next = Simplify
        .run(
            LayoutSchema
                .run(Validate::new(&bindings).run(&replacement).unwrap())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(extent(&next), Some(32));
}

struct InvalidSource;
impl Pass<hw_model_schema::analysis::ValidatedSchema<'_>> for InvalidSource {
    type Output = SchemaGraph;
    type Error = hw_model_schema::analysis::AnalysisError;
    fn run(
        self,
        input: hw_model_schema::analysis::ValidatedSchema<'_>,
    ) -> Result<Self::Output, Self::Error> {
        assert_eq!(input.into_source().definitions().count(), 1);
        let mut set = SchemaSet::new();
        set.add_definition(Definition::record::<Storage>(
            RecordSpec::new(4, 1, &[]),
            vec![hw_model_schema::ast::Field::new(
                "outside",
                4,
                4,
                hw_model_schema::Endian::Little,
                hw_model_schema::ast::FieldType::Scalar {
                    definition: hw_model_schema::FieldDefinition::new(
                        hw_model_schema::ScalarKind::U32,
                    ),
                    constant: None,
                },
            )],
        ));
        Resolve.run(set)
    }
}
#[test]
fn transformed_source_must_revalidate_before_generation() {
    use hw_model_schema::{
        analysis::AnalysisError,
        export::{Artifact, InMemoryArtifactSink, record, value},
    };
    struct Backend;
    impl record::Backend for Backend {}
    impl value::Backend for Backend {
        type Domain = record::Domain;
        type Error = Infallible;
        type Context<'a> = hw_model_schema::traversal::record::Context<'a>;
        fn finish(self) -> Result<Vec<Artifact>, Infallible> {
            Ok(vec![Artifact {
                logical_name: "output".into(),
                media_type: "text/plain".into(),
                contents: b"output".to_vec(),
            }])
        }
    }
    let bindings = Bindings::new();
    let original = Resolve.run(source(4)).unwrap();
    let graph = InvalidSource
        .run(Validate::new(&bindings).run(&original).unwrap())
        .unwrap();
    let mut sink = InMemoryArtifactSink::new();
    let mut constructed = false;
    let result = Pipeline::new(&graph)
        .run(Validate::new(&bindings))
        .and_then(|p| p.run(LayoutSchema))
        .and_then(|p| p.run(Simplify))
        .map(|p| {
            constructed = true;
            hw_model_schema::export::generate(p.input(), Backend, &mut sink).unwrap();
        });
    assert!(matches!(
        result,
        Err(AnalysisError::SourceField {
            field: "outside",
            ..
        })
    ));
    assert!(!constructed);
    assert!(sink.artifacts().is_empty());
}
