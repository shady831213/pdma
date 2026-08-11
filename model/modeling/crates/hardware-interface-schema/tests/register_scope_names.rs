//! Sibling-local register placement-name validation coverage.

use hardware_interface_schema::Endian;
use hardware_interface_schema::register::{RegisterDefinition, RegisterSpec};
use hardware_interface_schema::register_map::{
    RegisterMapError, RegisterMapSpec, RepeatedRegisterBank, ResolvedRegister,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisterId {
    Fixed,
    First,
    Second,
}

const VALUE: RegisterDefinition<u16> = RegisterDefinition::new("Value", 4, Endian::Little);

#[test]
fn identical_names_in_distinct_sibling_scopes_resolve_to_distinct_paths() {
    // Given
    const FIXED: [RegisterSpec<RegisterId>; 1] =
        [RegisterSpec::place(RegisterId::Fixed, "control", 0, VALUE)];
    const MEMBERS: [RegisterSpec<RegisterId>; 1] =
        [RegisterSpec::place(RegisterId::First, "control", 0, VALUE)];
    const BANKS: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("channel", 16, 2, 8, &MEMBERS)];

    // When
    let resolved = RegisterMapSpec::named("Device", 32, &FIXED, &BANKS).resolve();

    // Then
    let resolved = resolved.expect("names in distinct scopes must resolve");
    let paths = resolved
        .registers()
        .iter()
        .map(ResolvedRegister::path)
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "Device.control",
            "Device.channel[0].control",
            "Device.channel[1].control",
        ]
    );
    println!("resolved_paths={paths:?}");
}

#[test]
fn duplicate_names_within_fixed_scope_report_the_later_path() {
    // Given
    const FIXED: [RegisterSpec<RegisterId>; 2] = [
        RegisterSpec::place(RegisterId::Fixed, "control", 0, VALUE),
        RegisterSpec::place(RegisterId::First, "control", 4, VALUE),
    ];

    // When
    let result = RegisterMapSpec::named("Device", 16, &FIXED, &[]).resolve();

    // Then
    println!("fixed_scope_error={result:?}");
    assert_eq!(
        result,
        Err(RegisterMapError::DuplicatePlacementName {
            root: "Device",
            path: "Device.control".into(),
            declaration_index: 1,
            name: "control",
        })
    );
}

#[test]
fn duplicate_names_within_one_bank_scope_report_the_later_path() {
    // Given
    const MEMBERS: [RegisterSpec<RegisterId>; 2] = [
        RegisterSpec::place(RegisterId::First, "control", 0, VALUE),
        RegisterSpec::place(RegisterId::Second, "control", 4, VALUE),
    ];
    const BANKS: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("channel", 16, 1, 8, &MEMBERS)];

    // When
    let result = RegisterMapSpec::named("Device", 32, &[], &BANKS).resolve();

    // Then
    println!("bank_scope_error={result:?}");
    assert_eq!(
        result,
        Err(RegisterMapError::DuplicatePlacementName {
            root: "Device",
            path: "Device.channel[0].control".into(),
            declaration_index: 1,
            name: "control",
        })
    );
}
