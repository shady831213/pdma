//! Map definitions retain parameter relationships independently of instances.
use hw_model_schema::ast::{DefinitionBody, DescribeSchema, SchemaSet};
use hw_model_schema::{register_map, register_schema};

register_schema! {
    pub register Word { width: 4, endian: Little, access: ReadWrite,
        reset: Zero(0), writable_mask: 0xffff_ffff, fields: {} }
}
register_map! {
    pub map Channel { alignment: 64, placements: [pub status: Word] }
}

#[test]
fn invalid_instance_arguments_are_rejected_without_creating_a_binding()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a map bank requires a positive channel count.
    let mut set = SchemaSet::new();
    // When: an invalid zero count is supplied.
    let result = set.add_map_instance::<Device>("invalid", &[0]);
    // Then: geometry validation fails and the set remains unchanged.
    assert!(matches!(
        result,
        Err(hw_model_schema::ast::GraphError::InstanceLayout(_))
    ));
    let graph = set.resolve()?;
    assert!(graph.instances().is_empty());
    assert_eq!(graph.definitions().count(), 0);
    Ok(())
}

#[test]
fn distinct_instances_keep_their_arguments_without_duplicating_definitions()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one map type is used twice with different channel counts.
    let mut schemas = SchemaSet::new();
    schemas.add_map_instance::<Device>("small", &[1])?;
    schemas.add_map_instance::<Device>("large", &[3])?;
    // When: the combined selection is resolved.
    let graph = schemas.resolve()?;
    // Then: both layouts survive, while the type graph remains shared.
    assert_eq!(graph.definitions().count(), 3);
    let [large, small] = graph.instances() else {
        return Err("two instances expected".into());
    };
    assert_eq!(large.name(), "large");
    assert_eq!(large.arguments(), &[3]);
    assert_eq!(small.arguments(), &[1]);
    assert_eq!(large.definition(), small.definition());
    assert_eq!(large.geometry().extent, Device::new(3)?.extent());
    assert_eq!(small.geometry().extent, Device::new(1)?.extent());
    Ok(())
}
register_map! {
    pub map Device(channels: usize) { alignment: 64, placements: [
        pub discovery: Word, pub channels: Channel[channels],
    ] }
}

#[test]
fn parameterized_map_collects_definitions_without_selecting_an_instance_count()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the generic map definition is selected, with no bound count.
    let mut schemas = SchemaSet::new();
    schemas.add::<Device>();
    // When: dependencies are resolved.
    let graph = schemas.resolve()?;
    // Then: map and register nodes survive; the bank retains its count expression.
    assert_eq!(graph.definitions().count(), 3);
    let root = Device::schema_definition();
    let DefinitionBody::RegisterMap(map) = root.body() else {
        return Err("map expected".into());
    };
    assert_eq!(map.parameters(), &["channels"]);
    let geometry = map.resolve(&[3])?;
    let actual = Device::new(3)?;
    let bank = geometry.placements.get(1).ok_or("missing bank")?;
    assert_eq!(geometry.extent, actual.extent());
    assert_eq!(bank.offset, actual.channels().base());
    assert_eq!(bank.stride, actual.channels().stride());
    assert_eq!(bank.count, actual.channels().count());
    Ok(())
}
