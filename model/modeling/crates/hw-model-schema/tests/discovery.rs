//! Discovery and manual roots share exactly the same dependency graph.
#![cfg(feature = "discovery")]
use hw_model_schema::{ast::SchemaRegistry, record_schema};

mod selected {
    use super::record_schema;
    record_schema! { pub record Message { alignment: 4, endian: Little, fields: { data: u32 } } }
}
mod selected_neighbor {
    use super::record_schema;
    record_schema! { pub record Message { alignment: 4, endian: Little, fields: { other: u32 } } }
}

#[test]
fn module_selection_is_segment_bounded_and_manual_additions_are_idempotent()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: both modules have registered a distinct type with the same short name.
    let registry = SchemaRegistry::discover();
    let mut schemas = registry.select_module(concat!(module_path!(), "::selected"));
    schemas.add_record::<selected::Message>();
    // When: automatically and manually selected roots are resolved together.
    let graph = schemas.resolve()?;
    // Then: the similarly prefixed neighbor is not selected, nor is a root duplicated.
    assert_eq!(graph.definitions().count(), 1);
    assert_eq!(graph.roots().len(), 1);
    assert_eq!(
        registry
            .select_module(concat!(module_path!(), "::selected_neighbor"))
            .resolve()?
            .definitions()
            .count(),
        1
    );
    Ok(())
}
