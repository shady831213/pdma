//! Placement attachment preserves declared depth-first topology order.
use hw_model_schema::*;

fn bank(path: &str, offset: usize) -> ResolvedRegisterMapPlacement<u16> {
    ResolvedRegisterMapPlacement::new(
        path.into(),
        RegisterMapPlacement::new(0, "bank", offset, 1, 4, RegisterLayoutGeometry::new(4, 4))
            .as_bank(),
    )
}

#[test]
fn rejects_parent_metadata_after_its_child() {
    let map = finish_ordered_register_map::<u16>("Root", 4, vec![]).unwrap();
    let result = map.with_placements(vec![bank("Root.outer[0].inner", 0), bank("Root.outer", 0)]);
    assert!(
        matches!(result, Err(RegisterMapError::PlacementOrder { root: "Root", path, ancestor })
        if path == "Root.outer[0].inner" && ancestor == "Root.outer")
    );
}

#[test]
fn rejects_reentry_into_a_closed_subtree() {
    let map = finish_ordered_register_map::<u16>("Root", 8, vec![]).unwrap();
    let result = map.with_placements(vec![
        bank("Root.outer", 0),
        bank("Root.sibling", 4),
        bank("Root.outer[0].inner", 0),
    ]);
    assert!(
        matches!(result, Err(RegisterMapError::PlacementOrder { root: "Root", path, ancestor })
        if path == "Root.outer[0].inner" && ancestor == "Root.outer")
    );
}

#[test]
fn preserves_declaration_order_independent_of_physical_order() {
    let map = finish_ordered_register_map::<u16>("Root", 8, vec![]).unwrap();
    let map = map
        .with_placements(vec![
            bank("Root.sibling", 4),
            bank("Root.outer", 0),
            bank("Root.outer[0].inner", 0),
        ])
        .unwrap();
    let paths: Vec<_> = map.banks().iter().map(ResolvedRegisterBank::path).collect();
    assert_eq!(paths, ["Root.sibling", "Root.outer", "Root.outer[0].inner"]);
    let ancestry: Vec<_> = map
        .enclosing_banks("Root.outer[0].inner[0]")
        .map(|(_, bank, _)| bank.path())
        .collect();
    assert_eq!(ancestry, ["Root.outer", "Root.outer[0].inner"]);
}

#[test]
fn partial_opaque_topology_does_not_require_invented_ancestors() {
    let map = finish_ordered_register_map::<u16>("Root", 4, vec![]).unwrap();
    let map = map
        .with_placements(vec![bank("Root.outer[0].inner", 0)])
        .unwrap();
    assert_eq!(map.banks().len(), 1);
    assert!(map.registers().is_empty());
    let ancestry: Vec<_> = map
        .enclosing_banks("Root.outer[0].inner[0]")
        .map(|(_, bank, _)| bank.path())
        .collect();
    assert_eq!(ancestry, ["Root.outer[0].inner"]);
}
