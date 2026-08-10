//! Black-box register-map tests.

use hardware_interface_schema::register::{Access, RegisterSpec};
use hardware_interface_schema::register_map::{
    RegisterLocation, RegisterMapSpec, RegisterMapValidationError, RepeatedRegisterBank,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisterId {
    Control,
    Status,
    Data,
    Count,
}

const fn reg(
    id: RegisterId,
    name: &'static str,
    offset: usize,
    width: usize,
) -> RegisterSpec<RegisterId> {
    RegisterSpec::new(id, name, offset, width)
}

const FIXED: [RegisterSpec<RegisterId>; 1] =
    [reg(RegisterId::Control, "control", 0x08, 4).with_access(Access::ReadWrite)];
const MEMBERS: [RegisterSpec<RegisterId>; 2] = [
    reg(RegisterId::Data, "data", 0, 4),
    reg(RegisterId::Count, "count", 8, 8),
];
const BANKS: [RepeatedRegisterBank<RegisterId>; 1] = [RepeatedRegisterBank::new(
    "channel", 0x20, 2, 0x20, &MEMBERS,
)];

#[test]
fn map_locate_distinguishes_fixed_and_repeated_exact_starts() {
    // Given
    let map = RegisterMapSpec::new(0x80, &FIXED, &BANKS);

    // When
    let locations = [map.locate(0x08), map.locate(0x20), map.locate(0x48)];

    // Then
    assert_eq!(map.validate(), Ok(()));
    assert_eq!(map.size(), 0x80);
    assert_eq!(map.registers(), &FIXED);
    assert_eq!(map.banks(), &BANKS);
    assert_eq!(BANKS[0].name(), "channel");
    assert_eq!(BANKS[0].base(), 0x20);
    assert_eq!(BANKS[0].count(), 2);
    assert_eq!(BANKS[0].stride(), 0x20);
    assert_eq!(BANKS[0].registers(), &MEMBERS);
    assert_eq!(
        locations,
        [
            Some(RegisterLocation::Fixed {
                register: FIXED[0],
                offset: 0x08,
            }),
            Some(RegisterLocation::Repeated {
                bank_name: "channel",
                instance: 0,
                register: MEMBERS[0],
                offset: 0x20,
            }),
            Some(RegisterLocation::Repeated {
                bank_name: "channel",
                instance: 1,
                register: MEMBERS[1],
                offset: 0x48,
            }),
        ]
    );
}

#[test]
fn map_locate_rejects_interior_and_unmapped_offsets() {
    // Given
    let map = RegisterMapSpec::new(0x80, &FIXED, &BANKS);

    // When
    let locations = [map.locate(0x09), map.locate(0x44), map.locate(0x7f)];

    // Then
    assert_eq!(locations, [None, None, None]);
}

#[test]
fn map_validation_rejects_duplicate_id_before_name_and_start() {
    // Given
    const DUPLICATES: [RegisterSpec<RegisterId>; 2] = [
        reg(RegisterId::Status, "same", 0, 4),
        reg(RegisterId::Status, "same", 0, 4),
    ];

    // When
    let result = RegisterMapSpec::new(8, &DUPLICATES, &[]).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::DuplicateRegisterId {
            id: RegisterId::Status,
        })
    );
}

#[test]
fn map_validation_rejects_duplicate_name_before_start_and_width() {
    // Given
    const DUPLICATES: [RegisterSpec<RegisterId>; 2] = [
        reg(RegisterId::Control, "same", 0, 3),
        reg(RegisterId::Status, "same", 0, 4),
    ];

    // When
    let result = RegisterMapSpec::new(8, &DUPLICATES, &[]).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::DuplicateRegisterName { name: "same" })
    );
}

#[test]
fn map_validation_rejects_duplicate_expanded_start_before_width() {
    // Given
    const COLLIDING: [RegisterSpec<RegisterId>; 2] = [
        reg(RegisterId::Control, "first", 0, 3),
        reg(RegisterId::Status, "second", 0, 4),
    ];

    // When
    let result = RegisterMapSpec::new(8, &COLLIDING, &[]).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::DuplicateExpandedStart { offset: 0 })
    );
}

#[test]
fn map_validation_rejects_fixed_map_overflow() {
    // Given
    const TAIL: [RegisterSpec<RegisterId>; 1] = [reg(RegisterId::Status, "tail", 6, 4)];

    // When
    let result = RegisterMapSpec::new(8, &TAIL, &[]).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::RegisterOutOfBounds {
            name: "tail",
            offset: 6,
            width: 4,
            map_size: 8,
        })
    );
}

#[test]
fn map_validation_rejects_zero_and_misaligned_bank_stride() {
    // Given
    const ZERO: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("zero", 0, 1, 0, &MEMBERS)];
    const MISALIGNED: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("odd", 0, 1, 18, &MEMBERS)];

    // When
    let results = [
        RegisterMapSpec::new(0x40, &[], &ZERO).validate(),
        RegisterMapSpec::new(0x40, &[], &MISALIGNED).validate(),
    ];

    // Then
    assert_eq!(
        results[0],
        Err(RegisterMapValidationError::BankStrideZero { bank_name: "zero" })
    );
    assert_eq!(
        results[1],
        Err(RegisterMapValidationError::BankStrideMisaligned {
            bank_name: "odd",
            stride: 18,
            required_alignment: 8,
        })
    );
}

#[test]
fn map_validation_rejects_stride_smaller_than_member_extent() {
    // Given
    const WIDE_MEMBER: [RegisterSpec<RegisterId>; 1] = [reg(RegisterId::Data, "wide", 4, 8)];
    const SMALL: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("small", 0, 2, 8, &WIDE_MEMBER)];

    // When
    let result = RegisterMapSpec::new(0x40, &[], &SMALL).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::BankStrideTooSmall {
            bank_name: "small",
            stride: 8,
            required_extent: 12,
        })
    );
}

#[test]
fn map_validation_rejects_expanded_bank_map_overflow() {
    // Given
    const LARGE: [RepeatedRegisterBank<RegisterId>; 1] =
        [RepeatedRegisterBank::new("large", 0x30, 2, 0x20, &MEMBERS)];

    // When
    let result = RegisterMapSpec::new(0x5f, &[], &LARGE).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::RegisterOutOfBounds {
            name: "count",
            offset: 0x58,
            width: 8,
            map_size: 0x5f,
        })
    );
}

#[test]
fn map_validation_rejects_bank_expansion_arithmetic_overflow() {
    // Given
    const OFFSET_MEMBER: [RegisterSpec<RegisterId>; 1] =
        [reg(RegisterId::Data, "offset-data", 1, 4)];
    const OVERFLOWING: [RepeatedRegisterBank<RegisterId>; 1] = [RepeatedRegisterBank::new(
        "overflow",
        usize::MAX,
        1,
        8,
        &OFFSET_MEMBER,
    )];

    // When
    let result = RegisterMapSpec::new(usize::MAX, &[], &OVERFLOWING).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::BankExpansionOverflow {
            bank_name: "overflow",
            instance: 0,
            register_name: "offset-data",
        })
    );
}

#[test]
fn map_validation_rejects_fixed_bank_and_bank_bank_overlaps() {
    // Given
    const FIXED_OVERLAP: [RegisterSpec<RegisterId>; 1] =
        [reg(RegisterId::Control, "fixed", 0x22, 4)];
    const OTHER_MEMBERS: [RegisterSpec<RegisterId>; 2] = [
        reg(RegisterId::Control, "other-data", 0, 4),
        reg(RegisterId::Status, "other-count", 8, 8),
    ];
    const BANK_OVERLAP: [RepeatedRegisterBank<RegisterId>; 2] = [
        RepeatedRegisterBank::new("first", 0x20, 1, 0x20, &MEMBERS),
        RepeatedRegisterBank::new("second", 0x22, 1, 0x20, &OTHER_MEMBERS),
    ];

    // When
    let fixed_bank = RegisterMapSpec::new(0x80, &FIXED_OVERLAP, &BANKS).validate();
    let bank_bank = RegisterMapSpec::new(0x80, &[], &BANK_OVERLAP).validate();

    // Then
    assert!(matches!(
        fixed_bank,
        Err(RegisterMapValidationError::ExpandedOverlap {
            first_name: "fixed",
            second_name: "data",
            ..
        })
    ));
    assert!(matches!(
        bank_bank,
        Err(RegisterMapValidationError::ExpandedOverlap {
            first_name: "data",
            second_name: "other-data",
            ..
        })
    ));
}

#[test]
fn map_validation_prioritizes_register_shape_before_map_stride_and_overlap() {
    // Given
    const BAD: [RegisterSpec<RegisterId>; 1] = [reg(RegisterId::Data, "bad", 0, 3)];
    const BAD_BANKS: [RepeatedRegisterBank<RegisterId>; 1] = [RepeatedRegisterBank::new(
        "bad-bank",
        usize::MAX,
        1,
        0,
        &BAD,
    )];

    // When
    let result = RegisterMapSpec::new(1, &[], &BAD_BANKS).validate();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapValidationError::Register(
            hardware_interface_schema::register::RegisterValidationError::InvalidWidth {
                name: "bad",
                width: 3,
            }
        ))
    );
}
