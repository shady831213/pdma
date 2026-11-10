use super::*;
#[test]
fn referenced_fields_are_visited_once_per_owner_with_both_doc_fragments()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a reusable bits definition embedded in a register.
    let schema = schema()?;
    let mut sink = MemoryOutput::new();
    // When shared recursion explicitly follows the register's bits reference.
    sink.generate(&schema, Trace::default())
        .expect("generation");
    // Then reference metadata remains present, while the register owns one effective field visit.
    let text = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    assert_eq!(text.matches("field:enabled").count(), 1);
    assert_eq!(text.matches("short:BitField:enabled:enabled").count(), 2);
    assert_eq!(
        text.matches("full:BitField:enabled:bit first\nbit second")
            .count(),
        2
    );
    assert!(text.contains("short:Register:Control:register|full:Register:Control:register detail|width:4|register-alignment:4"));
    Ok(())
}
#[test]
fn documentation_failure_is_typed_and_publishes_no_partial_artifact()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a backend rejecting full descriptions after an accepted summary.
    let schema = schema()?;
    let mut sink = MemoryOutput::new();
    // When a descendant documentation callback fails.
    let result = sink.generate(
        &schema,
        Trace {
            fail: true,
            ..Trace::default()
        },
    );
    // Then the exact error escapes and the caller discards the failed root.
    assert_eq!(
        result,
        Err(export::ExportError::Render(Failure::Description))
    );
    assert!(sink.outputs().is_empty());
    Ok(())
}
#[test]
fn map_and_register_geometry_dispatch_have_distinct_owners()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a resolved map with completed constant geometry.
    let mut set = SchemaSet::new();
    set.add::<Device>();
    let schema = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let mut sink = MemoryOutput::new();
    // When shared traversal emits map formulas and concrete fields.
    sink.generate(&schema, Trace::default())
        .expect("generation");
    // Then semantic leaf callbacks receive numeric formulas and placement overrides only once.
    let text = String::from_utf8(
        sink.outputs()
            .iter()
            .flat_map(|output| output.bytes.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    assert!(
        text.contains(
            "map-alignment:4|short:Placement:control:placement|offset|value:0|value:1|value:4|value:4|value:4|extent|value:4"
        ),
        "{text}"
    );
    assert_eq!(text.matches("short:Placement:control:placement").count(), 1);
    assert_eq!(text.matches("short:Register:Control:register").count(), 1);
    Ok(())
}
