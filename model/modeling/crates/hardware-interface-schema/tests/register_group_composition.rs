//! Fixed schema-group composition coverage.

use hardware_interface_schema::{
    register_map, register_map::RegisterMapValidationError, register_schema,
};

register_schema! {
    pub registers GlobalRegisters<DeviceMapRegisterId> {
        status: { id: 1, name: "status", offset: 0, width: 4, access: ReadOnly,
            reset: Constant(0x1234), writable_mask: 0 },
    }
}

register_schema! {
    pub registers QueueRegisters<DeviceMapRegisterId> {
        command: { id: 2, name: "command", offset: 0x20, width: 8, access: ReadWrite,
            reset: Zero(0), writable_mask: 0xffff },
    }
}

register_map! {
    pub map DeviceMap {
        size: 0x100,
        fixed: {},
        fixed_groups: {
            GlobalRegisters: [status],
            QueueRegisters: [command],
        },
        banks: {}
    }
}

register_schema! {
    pub registers FirstDuplicateRegisters<DuplicateMapRegisterId> {
        first: { id: 7, name: "first", offset: 0x40, width: 4, access: ReadOnly,
            reset: Zero(0), writable_mask: 0 },
    }
}

register_schema! {
    pub registers SecondDuplicateRegisters<DuplicateMapRegisterId> {
        second: { id: 7, name: "first", offset: 0x40, width: 4, access: ReadOnly,
            reset: Zero(0), writable_mask: 0 },
    }
}

register_map! {
    pub map DuplicateMap {
        size: 0x100,
        fixed: {},
        fixed_groups: {
            FirstDuplicateRegisters: [first],
            SecondDuplicateRegisters: [second],
        },
        banks: {}
    }
}

#[test]
fn fixed_schema_groups_compose_without_repeating_register_metadata() {
    // Given: two independently declared groups sharing the map's generated identity.
    let expected = [GlobalRegisters::STATUS, QueueRegisters::COMMAND];

    // When: the combined map is queried by identity and exact register start.
    let actual = [
        DeviceMap::register(DeviceMapRegisterId::Status),
        DeviceMap::register(DeviceMapRegisterId::Command),
    ];

    // Then: the group metadata is reused verbatim by the map.
    assert_eq!(actual, expected);
    assert_eq!(DeviceMap::SPEC.validate(), Ok(()));
    assert_eq!(
        DeviceMap::SPEC.register_groups(),
        &[GlobalRegisters::SPEC, QueueRegisters::SPEC]
    );
    assert!(DeviceMap::locate(0).is_some());
    assert!(DeviceMap::locate(0x20).is_some());
    assert_eq!(DeviceMap::locate(1), None);
}

#[test]
fn fixed_schema_groups_reject_duplicate_explicit_numeric_ids_before_other_metadata() {
    // Given: distinct typed IDs in separate fixed groups share one explicit numeric schema ID.
    let map = DuplicateMap::SPEC;

    // When: the composed map validates all declarations together.
    let result = map.validate();

    // Then: numeric identity duplication wins before distinct names and starts are considered.
    assert_eq!(
        result,
        Err(RegisterMapValidationError::DuplicateRegisterId {
            id: DuplicateMapRegisterId::Second,
        })
    );
}
