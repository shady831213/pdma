use hardware_interface_schema as his;
use his::{RegisterAccessGroup, register_map, register_schema};

register_schema! { pub register Word { width: 4, endian: Little, access: ReadOnly,
    reset: Zero(0), writable_mask: 0, fields: {}, } }
register_map! { pub map Described { alignment: 4, placements: [
    magic: Word,
    expected_epoch: Word,
], access: [
    global.magic => magic,
    queue.command.expected_epoch => expected_epoch,
] } }

fn main() {
    let paths = Described::ACCESS_PATHS;
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0].groups(), &[RegisterAccessGroup::new("global")]);
    assert_eq!(paths[0].leaf().name(), "magic");
    assert_eq!(paths[0].leaf().target(), DescribedRegisterId::Magic);
    assert_eq!(
        paths[1].groups(),
        &[
            RegisterAccessGroup::new("queue"),
            RegisterAccessGroup::new("command"),
        ]
    );
    assert_eq!(paths[1].leaf().name(), "expected_epoch");
    assert_eq!(paths[1].leaf().target(), DescribedRegisterId::ExpectedEpoch);
    assert_eq!(Described::SPEC.placements()[0].name(), "magic");
    assert_eq!(Described::resolve().unwrap().registers()[0].offset(), 0);
}
