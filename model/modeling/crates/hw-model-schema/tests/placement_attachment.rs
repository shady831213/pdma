//! Checked metadata attachment must reject malformed topology without panicking.
use hw_model_schema::*;

register_schema! { pub register Word {
    width: 4, endian: Little, access: ReadWrite, reset: Zero(0),
    writable_mask: 0xffff_ffff, fields: {}
} }
register_map! { pub map Map { alignment: 4, placements: [pub words: Word[2]] } }

#[test]
fn malformed_bank_base_does_not_panic() {
    let map = Map::resolve().unwrap();
    let placement = RegisterMapPlacement::new(
        map.registers().first().unwrap().id(),
        "words",
        4,
        1,
        4,
        RegisterLayoutGeometry::new(4, 4),
    )
    .as_bank();
    let result = std::panic::catch_unwind(|| {
        map.with_placements(vec![ResolvedRegisterMapPlacement::new(
            "Map.words".into(),
            placement,
        )])
    });
    assert!(matches!(
        result.unwrap(),
        Err(RegisterMapError::PlacementGeometry {
            source: RegisterBankError::Extent,
            ..
        })
    ));
}

#[test]
fn rejects_invalid_bank_geometry_and_leaf_membership() {
    for (offset, count, stride, alignment, extent, expected) in [
        (0, 0, 4, 4, 4, RegisterBankError::ZeroCount),
        (0, 2, 4, 0, 4, RegisterBankError::Alignment),
        (1, 2, 4, 4, 4, RegisterBankError::Alignment),
        (0, 2, 3, 4, 4, RegisterBankError::Alignment),
        (0, 2, 0, 4, 4, RegisterBankError::Overlap),
        (0, 2, 4, 4, 0, RegisterBankError::Extent),
        (0, 1, 4, 4, 4, RegisterBankError::Extent),
        (0, 2, 4, 4, 2, RegisterBankError::Extent),
        (0, usize::MAX, 4, 4, 4, RegisterBankError::Overflow),
    ] {
        let map = Map::resolve().unwrap();
        let placement = RegisterMapPlacement::new(
            map.registers().first().unwrap().id(),
            "words",
            offset,
            count,
            stride,
            RegisterLayoutGeometry::new(alignment, extent),
        )
        .as_bank();
        let result = map.with_placements(vec![ResolvedRegisterMapPlacement::new(
            "Map.words".into(),
            placement,
        )]);
        assert!(
            matches!(result, Err(RegisterMapError::PlacementGeometry { root: "Map", path, source })
            if path == "Map.words" && source == expected)
        );
    }
}

#[test]
fn rejects_unrelated_and_duplicate_placement_paths() {
    for paths in [
        vec!["Other.words"],
        vec!["Map.other"],
        vec!["Map.words", "Map.words"],
    ] {
        let map = Map::resolve().unwrap();
        let placement = RegisterMapPlacement::new(
            map.registers().first().unwrap().id(),
            "words",
            0,
            2,
            4,
            RegisterLayoutGeometry::new(4, 4),
        )
        .as_bank();
        let result = map.with_placements(
            paths
                .into_iter()
                .map(|path| ResolvedRegisterMapPlacement::new(path.into(), placement))
                .collect(),
        );
        assert!(matches!(
            result,
            Err(RegisterMapError::PlacementGeometry { .. })
        ));
    }
}

#[test]
fn valid_attachment_retains_bank_templates_and_ancestry() {
    let map = Map::resolve().unwrap();
    let placement = RegisterMapPlacement::new(
        map.registers().first().unwrap().id(),
        "words",
        0,
        2,
        4,
        RegisterLayoutGeometry::new(4, 4),
    )
    .as_bank();
    let resolved = map
        .with_placements(vec![ResolvedRegisterMapPlacement::new(
            "Map.words".into(),
            placement,
        )])
        .unwrap();
    let bank = resolved.banks().first().unwrap();
    assert_eq!((bank.base(), bank.count(), bank.stride()), (0, 2, 4));
    assert_eq!(bank.registers().first().unwrap().offset(), 0);
    assert_eq!(bank.expanded_registers().len(), 2);
    for (index, leaf) in resolved.registers().iter().enumerate() {
        assert_eq!(leaf.bank_ancestry().first().unwrap().path(), "Map.words");
        assert_eq!(leaf.bank_ancestry().first().unwrap().index(), index);
    }
}

#[test]
fn leafless_opaque_banks_keep_their_reserved_geometry() {
    let map = finish_ordered_register_map::<u16>("Opaque", 32, vec![]).unwrap();
    let bank = RegisterMapPlacement::new(0, "windows", 4, 2, 8, RegisterLayoutGeometry::new(4, 8))
        .as_bank();
    let instances = [
        ResolvedRegisterMapPlacement::new("Opaque.windows".into(), bank),
        ResolvedRegisterMapPlacement::new(
            "Opaque.windows[0]".into(),
            RegisterMapPlacement::new(0, "window", 4, 1, 0, RegisterLayoutGeometry::new(4, 8)),
        ),
        ResolvedRegisterMapPlacement::new(
            "Opaque.windows[1]".into(),
            RegisterMapPlacement::new(0, "window", 12, 1, 0, RegisterLayoutGeometry::new(4, 8)),
        ),
    ];
    let resolved = map.with_placements(instances.to_vec()).unwrap();
    assert!(resolved.registers().is_empty());
    assert_eq!(resolved.placements().len(), 2);
    assert_eq!(
        (
            resolved.banks().first().unwrap().base(),
            resolved.banks().first().unwrap().count(),
            resolved.banks().first().unwrap().stride()
        ),
        (4, 2, 8)
    );
}

#[test]
fn empty_scalar_child_maps_remain_valid() {
    let map = finish_ordered_register_map::<u16>("EmptyParent", 0, vec![]).unwrap();
    let placement =
        RegisterMapPlacement::new(0, "empty", 0, 1, 0, RegisterLayoutGeometry::new(4, 0));
    let empty = map
        .with_placements(vec![ResolvedRegisterMapPlacement::new(
            "EmptyParent.empty".into(),
            placement,
        )])
        .unwrap();
    assert_eq!(empty.size(), 0);
    assert!(empty.registers().is_empty());
    assert_eq!(empty.placements().first().unwrap().placement().extent(), 0);
}

#[test]
fn empty_scalar_metadata_occupies_no_bytes_inside_a_register() {
    static REGISTERS: [RegisterSpec<u16>; 1] = [RegisterSpec::new(0, "word", 0, 8)];
    let map = RegisterMapSpec::named("Root", 8, &REGISTERS, &[])
        .resolve()
        .unwrap();
    let placement =
        RegisterMapPlacement::new(1, "empty", 4, 1, 0, RegisterLayoutGeometry::new(4, 0));
    let resolved = map
        .with_placements(vec![ResolvedRegisterMapPlacement::new(
            "Root.empty".into(),
            placement,
        )])
        .unwrap();
    assert_eq!(resolved.registers().first().unwrap().offset(), 0);
    assert_eq!(
        resolved.placements().first().unwrap().placement().offset(),
        4
    );
}
