//! Source-field validation is independent of physical storage lowering.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::analysis::{AnalysisError, AnalyzedSchema};
use hw_model_schema::ast::{
    Definition, DefinitionId, Field, FieldType, GraphError, RecordDefinition, Reference,
    ReferenceKind,
};
use hw_model_schema::export::record::Context as RecordContext;
use hw_model_schema::export::{record::Backend, value};
use hw_model_schema::{Endian, FieldDefinition, RecordSpec, ScalarKind};
use output_support::MemoryOutput;

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
    assert!(AnalyzedSchema::record("valid", valid).is_ok());
    let outside = RecordDefinition::new(storage, vec![field("outside", 6)]);
    assert!(AnalyzedSchema::record("outside", outside).is_err());
    let overlap = RecordDefinition::new(storage, vec![field("left", 0), field("right", 2)]);
    assert!(AnalyzedSchema::record("overlap", overlap).is_err());
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
    fn root(
        &mut self,
        ctx: RecordContext<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        {
            let event: String = ctx.root_name().to_owned();
            if !self.0.is_empty() {
                writer.write_all(b"\n")?;
            }
            writer.write_all(event.as_bytes())?;
            self.0.push(event);
        }
        Ok(())
    }
}
impl value::Backend for DefinitionList {
    type Domain = hw_model_schema::export::record::Domain;
    type Error = std::io::Error;
    type Context<'a> = RecordContext<'a>;
}
impl hw_model_schema::export::OutputBackend for DefinitionList {
    fn metadata(
        &mut self,
        ctx: &Self::Context<'_>,
    ) -> Result<hw_model_schema::export::OutputMetadata, Self::Error> {
        Ok(hw_model_schema::export::OutputMetadata {
            logical_name: ctx.root_name().to_owned(),
            media_type: "text/plain".into(),
        })
    }
}

#[test]
fn manual_record_emits_transitive_dependencies_once_before_its_own_definition() {
    // Given: two fields share a dependency which itself references another record.
    let analysis = AnalyzedSchema::record("Manual", manual_record()).expect("valid record");
    let mut sink = MemoryOutput::new();
    // When: a downstream backend generates the selected manual record.
    sink.generate(&analysis, DefinitionList::default())
        .expect("generation");
    // Then: both dependencies are emitted once before their users.
    assert_eq!(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
        b"Leaf\nMiddle\nManual"
    );
}

#[test]
fn manual_record_keeps_each_reference_to_a_shared_dependency() {
    // Given: the same type occurs in two independently named fields.
    let record = manual_record();
    // When: the manual record is analyzed.
    let analysis = AnalyzedSchema::record("Manual", record).expect("valid record");
    // Then: deduplicating definitions does not discard referring field identities.
    let root = analysis.units().last().expect("root");
    let edges: Vec<_> = root
        .references
        .iter()
        .map(|r| (r.name, r.kind, r.target.clone()))
        .collect();
    assert_eq!(
        edges,
        vec![
            (
                "left",
                ReferenceKind::Field,
                analysis
                    .definition(DefinitionId::of::<Middle>())
                    .expect("middle")
                    .id
                    .clone()
            ),
            (
                "right",
                ReferenceKind::Field,
                analysis
                    .definition(DefinitionId::of::<Middle>())
                    .expect("middle")
                    .id
                    .clone()
            ),
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
    let result = AnalyzedSchema::record("Manual", record);
    // Then: invalid dependencies cannot reach emission.
    assert!(
        matches!(result, Err(AnalysisError::Dependencies(GraphError::Cycle(path)))
        if path.first().is_some_and(|edge| edge.ends_with("Recursive.child")))
    );
}
