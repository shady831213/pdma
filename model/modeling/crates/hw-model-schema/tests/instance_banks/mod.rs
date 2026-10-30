//! Constructor-bound banks and indexed singleton behavior.
use hw_model_schema::{register_map, register_schema};
mod contiguous;
#[cfg(feature = "export-c-header")]
mod export;
mod routing;

#[test]
fn enclosing_banks_preserve_topology_after_all_registers_are_hidden()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: permission projection hides every register but retains bank topology.
    let map = Nested::new(3, 1)?.resolve()?.project_access(|_| None)?;
    // When: a backend requests the enclosing banks of a nested singleton leaf.
    let ancestors: Vec<_> = map
        .enclosing_banks("Nested.groups[2].lanes[0].word")
        .map(|(position, bank, instance)| (position, bank.path(), instance))
        .collect();
    // Then: ancestry uses declaration identity, including empty and singleton banks.
    assert_eq!(
        ancestors,
        [(0, "Nested.groups", 2), (3, "Nested.groups[2].lanes", 0)]
    );
    assert!(
        map.enclosing_banks("Nested.groups[3].word")
            .next()
            .is_none()
    );
    assert!(
        map.enclosing_banks("Nested.groups_extra[0].word")
            .next()
            .is_none()
    );
    assert!(
        map.enclosing_banks("Nested.groups[0]suffix")
            .next()
            .is_none()
    );
    Ok(())
}

register_schema! { pub register Word {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
} }
register_map! { pub map Channel {
    alignment: 4, extent: 16, placements: [pub word: Word],
} }
register_map! { pub map Device(channels: usize) {
    alignment: 4, placements: [
        pub control: Channel,
        pub channels: Channel[channels] { offset: 16 },
        pub suffix: Channel,
    ],
} }
register_map! { pub map Single {
    alignment: 4, placements: [pub channels: Channel[1]],
} }
register_map! { pub map Group(lanes: usize) {
    alignment: 4, placements: [pub lanes: Channel[lanes]],
} }
register_map! { pub map Nested(groups: usize, lanes: usize) {
    alignment: 4, placements: [pub groups: Group(lanes)[groups], pub tail: Channel],
} }

#[test]
fn topology_when_banks_are_nested_and_projected() {
    let layout = Nested::new(3, 1).unwrap();
    let resolved = layout.resolve().unwrap();
    let projected = resolved.project_access(|_| None).unwrap();
    assert_eq!(layout.groups().child().lanes().count(), 1);
    assert_eq!(resolved.banks().len(), 4);
    assert_eq!(resolved.banks().first().unwrap().count(), 3);
    assert_eq!(resolved.banks().first().unwrap().registers().len(), 1);
    assert_eq!(projected.banks().len(), 4);
    assert_eq!(projected.banks().first().unwrap().count(), 3);
    assert!(projected.banks().first().unwrap().registers().is_empty());
    let ancestry = resolved.registers().get(2).unwrap().bank_ancestry();
    assert_eq!(ancestry.len(), 2);
    assert_eq!(ancestry.first().unwrap().path(), "Nested.groups");
    assert_eq!(ancestry.first().unwrap().index(), 2);
    assert_eq!(ancestry.get(1).unwrap().path(), "Nested.groups[2].lanes");
    assert_eq!(ancestry.get(1).unwrap().index(), 0);
}

#[test]
fn geometry_when_counts_are_bound_to_instances() {
    // Given independent constructor counts.
    for count in [1, 3, 5] {
        // When a layout is constructed.
        let layout = Device::new(count).unwrap();
        // Then routing and suffix inference use occupied extent.
        assert_eq!(layout.extent(), 16 + count * 16 + 16);
        assert_eq!(
            layout.channels().offset(count - 1),
            Some(16 + (count - 1) * 16)
        );
        assert_eq!(layout.channels().offset(count), None);
        assert_eq!(layout.channels().locate(16), Some((0, 0)));
        assert_eq!(layout.channels().locate(20), Some((0, 4)));
        assert_eq!(layout.resolve().unwrap().registers().len(), count + 2);
    }
}

#[test]
fn indexed_identity_when_bank_has_one_element() {
    // Given an explicitly indexed singleton bank.
    let resolved = Single::resolve().unwrap();
    // When its generated metadata and typed view are used.
    let bytes = [0; 16];
    let view = SingleView::new(&bytes).unwrap();
    // Then it retains index identity and the indexed accessor.
    assert_eq!(
        resolved.registers().first().unwrap().path(),
        "Single.channels[0].word"
    );
    assert_eq!(
        view.channels(0).unwrap().word().unwrap().value().unwrap(),
        0
    );
}
