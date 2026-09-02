//! Runtime-neutral access-path descriptor behavior.

use hardware_interface_schema::{
    RegisterAccessGroup, RegisterAccessLeaf, RegisterAccessPath, register_map, register_schema,
};

register_schema! { register Word { width: 4, endian: Little, access: ReadOnly,
reset: Zero(0), writable_mask: 0, fields: {}, } }
register_map! { map Described { alignment: 4, placements: [
    magic: Word,
    expected_epoch: Word,
], access: [
    global.magic => magic,
    queue.command.expected_epoch => expected_epoch,
] } }
register_map! { map Undescribed { alignment: 4, placements: [word: Word] } }

const GLOBAL: &[RegisterAccessGroup] = &[RegisterAccessGroup::new("global")];
const QUEUE_COMMAND: &[RegisterAccessGroup] = &[
    RegisterAccessGroup::new("queue"),
    RegisterAccessGroup::new("command"),
];
const EXPECTED_PATHS: &[RegisterAccessPath<DescribedRegisterId>] = &[
    RegisterAccessPath::new(
        GLOBAL,
        RegisterAccessLeaf::new("magic", DescribedRegisterId::Magic),
    ),
    RegisterAccessPath::new(
        QUEUE_COMMAND,
        RegisterAccessLeaf::new("expected_epoch", DescribedRegisterId::ExpectedEpoch),
    ),
];

#[test]
fn descriptor_metadata_preserves_declared_path_and_placement_order() {
    let paths = Described::ACCESS_PATHS;

    assert_eq!(paths, EXPECTED_PATHS);
    assert_eq!(
        Described::SPEC.placements().first().map(|item| item.name()),
        Some("magic")
    );
    assert_eq!(
        Described::resolve()
            .unwrap()
            .registers()
            .first()
            .map(hardware_interface_schema::ResolvedRegister::offset),
        Some(0)
    );
}

#[test]
fn optional_access_block_leaves_existing_maps_undescribed() {
    assert!(Undescribed::ACCESS_PATHS.is_empty());
    assert_eq!(
        Undescribed::SPEC
            .placements()
            .first()
            .map(|item| item.name()),
        Some("word")
    );
    assert_eq!(UndescribedRegisterId::ALL, [UndescribedRegisterId::Word]);
}
