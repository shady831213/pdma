//! Register traversal preserves structural paths and checked layout semantics.
use hw_model_schema::{
    graph::{DefinitionBody, DescribeSchema, LayoutError},
    register_map, register_schema,
};

register_schema! {
    pub register Word { width: 4, endian: Big, access: ReadOnly,
        reset: Zero(7), writable_mask: 0, fields: {} }
}
register_map! {
    pub map Leaf { alignment: 16, placements: [pub status: Word] }
}
register_map! {
    pub map Group(n: usize) { alignment: 16, placements: [
        pub scalar: Word, pub lanes: Leaf[n],
    ] }
}
register_map! {
    pub map Root(n: usize) { alignment: 16, placements: [
        pub header: Word, pub groups: Group(n)[2],
    ] }
}

#[test]
fn register_walk_preserves_nested_bank_paths_and_absolute_addresses()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two parameterized blocks each contain a scalar and a bank of one.
    let definition = Root::schema_definition();
    let DefinitionBody::RegisterMap(map) = definition.body() else {
        return Err("map expected".into());
    };
    // When: the generic instance walker expands their register leaves.
    let registers = map.registers(&[1])?;
    // Then: structural identity and nested alignment survive without target naming policy.
    let observed: Vec<_> = registers
        .iter()
        .map(|register| {
            (
                register
                    .path()
                    .iter()
                    .map(|part| (part.name, part.index))
                    .collect::<Vec<_>>(),
                register.offset(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        vec![
            (vec![("header", None)], 0),
            (vec![("groups", Some(0)), ("scalar", None)], 16),
            (
                vec![("groups", Some(0)), ("lanes", Some(0)), ("status", None)],
                32
            ),
            (vec![("groups", Some(1)), ("scalar", None)], 48),
            (
                vec![("groups", Some(1)), ("lanes", Some(0)), ("status", None)],
                64
            ),
        ]
    );
    for register in &registers {
        assert_eq!(
            register.definition(),
            hw_model_schema::graph::DefinitionId::of::<Word>()
        );
        assert_eq!(register.register().endian(), hw_model_schema::Endian::Big);
        assert_eq!(register.register().reset_value(), 7);
    }
    Ok(())
}

#[test]
fn register_walk_rejects_invalid_instances_before_returning_leaves()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: zero is not a valid bank count, although the root has an earlier scalar.
    let definition = Root::schema_definition();
    let DefinitionBody::RegisterMap(map) = definition.body() else {
        return Err("map expected".into());
    };
    // When: resolution encounters the invalid child.
    let result = map.registers(&[0]);
    // Then: no partial traversal is returned.
    assert!(matches!(result, Err(LayoutError::Geometry)));
    Ok(())
}
