//! Malformed graphs fail with provenance rather than silently dropping definitions.
use hw_model_schema::{
    RecordSpec,
    ast::{Definition, GraphError, Reference, ReferenceKind, SchemaSet},
};

struct Recursive;
fn recursive() -> Definition {
    Definition::opaque::<Recursive>(RecordSpec::named("Recursive", 4, 4, &[], &[])).with_references(
        vec![Reference::new("child", ReferenceKind::Field, recursive)],
    )
}

#[test]
fn recursive_by_value_dependency_reports_its_field_path() {
    // Given: a custom definition points back to itself through a by-value field.
    let mut set = SchemaSet::new();
    set.add_definition(recursive());
    // When: dependency resolution follows that field.
    let result = set.resolve();
    // Then: the cycle is diagnosed with the reference path rather than looping.
    assert!(
        matches!(result, Err(GraphError::Cycle(path)) if path.first().is_some_and(|edge| edge.ends_with("Recursive.child")))
    );
}
