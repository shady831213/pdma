//! Source-field validation is independent of physical storage lowering.
use hw_model_schema::ast::{
    Definition, DefinitionId, Field, FieldType, GraphError, Node, RecordDefinition, Reference,
    ReferenceKind,
    analysis::{Analysis, AnalysisError},
};
use hw_model_schema::export::{Artifact, Backend, InMemoryArtifactSink, Scope, generate};
use hw_model_schema::{Endian, FieldDefinition, RecordSpec, ScalarKind};

const fn field(name: &'static str, offset: usize) -> Field {
    Field::new(
        name,
        offset,
        4,
        Endian::Little,
        FieldType::Scalar {
            definition: FieldDefinition::new(ScalarKind::U32),
            constant: None,
        },
    )
}

#[test]
fn manual_source_fields_are_checked_independently_of_storage_lowering() {
    let storage = RecordSpec::new(8, 4, &[]);
    let valid = RecordDefinition::new(storage, vec![field("left", 0), field("right", 4)]);
    assert!(Analysis::record("valid", valid).is_ok());
    let outside = RecordDefinition::new(storage, vec![field("outside", 6)]);
    assert!(Analysis::record("outside", outside).is_err());
    let overlap = RecordDefinition::new(storage, vec![field("left", 0), field("right", 2)]);
    assert!(Analysis::record("overlap", overlap).is_err());
}

struct Leaf;
struct Middle;
struct Recursive;

fn leaf() -> Definition {
    Definition::record::<Leaf>(
        RecordSpec::named("Leaf", 4, 4, &[], &[]),
        vec![field("value", 0)],
    )
}

fn reference(name: &'static str, offset: usize, target: fn() -> Definition) -> Field {
    Field::new(
        name,
        offset,
        4,
        Endian::Little,
        FieldType::Reference(Reference::new(name, ReferenceKind::Field, target)),
    )
}

fn middle() -> Definition {
    Definition::record::<Middle>(
        RecordSpec::named("Middle", 4, 4, &[], &[]),
        vec![reference("leaf", 0, leaf)],
    )
}

fn recursive() -> Definition {
    Definition::record::<Recursive>(
        RecordSpec::named("Recursive", 4, 4, &[], &[]),
        vec![reference("child", 0, recursive)],
    )
}

fn manual_record() -> RecordDefinition {
    RecordDefinition::new(
        RecordSpec::new(8, 4, &[]),
        vec![reference("left", 0, middle), reference("right", 4, middle)],
    )
}

#[derive(Default)]
struct DefinitionList(Vec<String>);
impl Backend for DefinitionList {
    type Error = std::convert::Infallible;
    fn definition(&mut self, _: Node<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        self.0.push(scope.unit.name.clone());
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "definitions.txt".into(),
            media_type: "text/plain".into(),
            contents: self.0.join("\n").into_bytes(),
        }])
    }
}

#[test]
fn manual_record_emits_transitive_dependencies_once_before_its_own_definition() {
    // Given: two fields share a dependency which itself references another record.
    let analysis = Analysis::record("Manual", manual_record()).expect("valid record");
    let mut sink = InMemoryArtifactSink::new();
    // When: a downstream backend generates the selected manual record.
    generate(&analysis, DefinitionList::default(), &mut sink).expect("generation");
    // Then: both dependencies are emitted once before their users.
    assert_eq!(
        sink.artifacts().first().expect("definition list").contents,
        b"Leaf\nMiddle\nManual"
    );
}

#[test]
fn manual_record_keeps_each_reference_to_a_shared_dependency() {
    // Given: the same type occurs in two independently named fields.
    let record = manual_record();
    // When: the manual record is analyzed.
    let analysis = Analysis::record("Manual", record).expect("valid record");
    // Then: deduplicating definitions does not discard referring field identities.
    let root = analysis.definitions().last().expect("root");
    let edges: Vec<_> = root
        .references
        .iter()
        .map(|r| (r.name, r.kind, r.target))
        .collect();
    assert_eq!(
        edges,
        vec![
            ("left", ReferenceKind::Field, DefinitionId::of::<Middle>()),
            ("right", ReferenceKind::Field, DefinitionId::of::<Middle>()),
        ]
    );
}

#[test]
fn manual_record_rejects_cycles_in_referenced_definitions() {
    // Given: a manual record references a type with a recursive by-value field.
    let record = RecordDefinition::new(
        RecordSpec::new(4, 4, &[]),
        vec![reference("root", 0, recursive)],
    );
    // When: analysis follows its dependencies.
    let result = Analysis::record("Manual", record);
    // Then: invalid dependencies cannot reach emission.
    assert!(
        matches!(result, Err(AnalysisError::Dependencies(GraphError::Cycle(path)))
        if path.first().is_some_and(|edge| edge.ends_with("Recursive.child")))
    );
}
