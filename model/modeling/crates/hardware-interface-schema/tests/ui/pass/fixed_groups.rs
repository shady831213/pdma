use hardware_interface_schema::{register_map, register_schema};

register_schema! { pub registers FirstRegisters<GroupMapRegisterId> {
    status: { id: 1, name: "status", offset: 0, width: 4, access: ReadOnly,
        reset: Zero(0), writable_mask: 0 },
} }
register_schema! { pub registers SecondRegisters<GroupMapRegisterId> {
    command: { id: 2, name: "command", offset: 8, width: 8, access: ReadWrite,
        reset: Constant(0), writable_mask: 0xffff },
} }
register_map! { pub map GroupMap {
    size: 32,
    fixed: {},
    fixed_groups: {
        FirstRegisters: [status],
        SecondRegisters: [command],
    },
    banks: {}
} }

fn main() {
    assert_eq!(GroupMap::SPEC.register_groups().len(), 2);
    assert_eq!(GroupMap::register(GroupMapRegisterId::Command), SecondRegisters::COMMAND);
}
