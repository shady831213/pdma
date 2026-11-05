//! Definition instances retain concrete geometry and structural bank membership.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings, Layout},
    ast::{DefinitionId, SchemaSet},
    register_map, register_schema,
};

register_schema! { pub register Word { width: 4, endian: Big, access: ReadOnly,
reset: Zero(7), writable_mask: 0, fields: {} } }
register_map! { pub map Leaf { alignment: 16, placements: [pub word: Word] } }
register_map! { pub map Group(n: usize) { alignment: 16, placements: [
    pub header: Word, pub leaves: Leaf[n],
] } }
register_map! { pub map Root(n: usize) { alignment: 16, placements: [
    pub groups: Group(n)[2], pub tail: Word,
] } }
register_map! { pub map Empty { alignment: 4, extent: 8, placements: [] } }
register_map! { pub map EmptyBanks { alignment: 4, placements: [pub empty: Empty[2]] } }

#[test]
fn definition_instance_keeps_nested_membership_and_partial_last_extent()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a bank of two groups each containing a singleton bank and a scalar.
    let mut set = SchemaSet::new();
    set.add::<Root>();
    let mut bindings = Bindings::new();
    bindings.bind::<Root>(&[1]);
    // When: the definition graph is prepared with concrete bindings.
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &bindings)?;
    let unit = prepared
        .definition(DefinitionId::of::<Root>())
        .ok_or("root")?;
    let Layout::Map(map) = &unit.body else {
        return Err("map".into());
    };
    let view = map.instance.as_ref().ok_or("instance")?;
    // Then: ordering, template-relative offsets and parent identities retain occupied geometry.
    assert_eq!(view.extent, 56);
    assert_eq!(
        view.registers
            .iter()
            .map(|r| (r.path.as_str(), r.offset))
            .collect::<Vec<_>>(),
        [
            ("Root.groups[0].header", 0),
            ("Root.groups[0].leaves[0].word", 16),
            ("Root.groups[1].header", 32),
            ("Root.groups[1].leaves[0].word", 48),
            ("Root.tail", 52),
        ]
    );
    assert_eq!(
        view.banks
            .iter()
            .map(|b| (b.path.as_str(), b.parent))
            .collect::<Vec<_>>(),
        [
            ("Root.groups", None),
            ("Root.groups[0].leaves", Some((0, 0))),
            ("Root.groups[1].leaves", Some((0, 1))),
        ]
    );
    let outer = view.banks.first().ok_or("outer")?;
    assert_eq!(
        outer.registers.iter().map(|r| r.offset).collect::<Vec<_>>(),
        [0, 16]
    );
    assert_eq!(
        outer
            .instances
            .iter()
            .map(|rs| rs.iter().map(|r| r.offset).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec![0, 16], vec![32, 48]]
    );
    let word = view.registers.get(3).ok_or("word")?;
    assert_eq!(
        word.segments
            .iter()
            .map(|s| (s.name.as_str(), s.index))
            .collect::<Vec<_>>(),
        [("groups", Some(1)), ("leaves", Some(0)), ("word", None)]
    );
    assert_eq!(word.register.endian(), hw_model_schema::Endian::Big);
    assert_eq!(word.register.reset_value(), 7);
    assert_eq!(word.register.access(), hw_model_schema::Access::ReadOnly);
    Ok(())
}

#[test]
fn definition_instance_retains_banks_with_empty_child_blocks()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two occupied child blocks contain no register leaves.
    let mut set = SchemaSet::new();
    set.add::<EmptyBanks>();
    // When: their definition graph is prepared.
    let prepared = AnalyzedSchema::from_graph(&set.resolve()?, &Bindings::new())?;
    let unit = prepared
        .definition(DefinitionId::of::<EmptyBanks>())
        .ok_or("root")?;
    let Layout::Map(map) = &unit.body else {
        return Err("map".into());
    };
    let view = map.instance.as_ref().ok_or("instance")?;
    // Then: empty leaf collections preserve each bank instance and the reserved extent.
    assert_eq!(view.extent, 16);
    assert!(view.registers.is_empty());
    assert_eq!(view.banks.len(), 1);
    let bank = view.banks.first().ok_or("bank")?;
    assert!(bank.registers.is_empty());
    assert_eq!(bank.instances.len(), 2);
    assert!(bank.instances.iter().all(Vec::is_empty));
    Ok(())
}
