//! Reusable register definition, placement, resolution, and checked-view coverage.

use hw_model_schema::register::{
    Access, RegisterDefinition, RegisterFieldDefinition, RegisterFieldSpec, RegisterValueError,
    ResetKind,
};
use hw_model_schema::register_map::{
    RegisterMapError, RegisterMapSpec, RegisterSpec, RepeatedRegisterBank,
};
use hw_model_schema::{Endian, SchemaMemberIdentity};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldId {
    Enable,
    Mode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegisterId {
    Primary,
    Mirror,
    Channel,
}

const FIELDS: [RegisterFieldSpec<FieldId>; 2] = [
    RegisterFieldSpec::place(
        FieldId::Enable,
        "enable",
        0,
        RegisterFieldDefinition::new(1),
    ),
    RegisterFieldSpec::place(FieldId::Mode, "mode", 4, RegisterFieldDefinition::new(3)),
];
const CONTROL: RegisterDefinition<FieldId> = RegisterDefinition::new("Control", 2, Endian::Little)
    .with_access(Access::ReadWrite)
    .with_reset(ResetKind::Constant, 0x20)
    .with_writable_mask(0x71)
    .with_fields(&FIELDS);
const FIXED: [RegisterSpec<RegisterId, FieldId>; 2] = [
    RegisterSpec::place(RegisterId::Primary, "primary", 0, CONTROL),
    RegisterSpec::place(RegisterId::Mirror, "mirror", 0x10, CONTROL),
];
const MEMBERS: [RegisterSpec<RegisterId, FieldId>; 1] = [RegisterSpec::place(
    RegisterId::Channel,
    "control",
    0,
    CONTROL,
)];
const BANKS: [RepeatedRegisterBank<RegisterId, FieldId>; 1] = [RepeatedRegisterBank::new(
    "channel", 0x20, 2, 0x10, &MEMBERS,
)];

#[test]
fn register_definition_surface() {
    // Given: one offset-free definition is reused by fixed and repeated placements.
    let map = RegisterMapSpec::named("Device", 0x40, &FIXED, &BANKS);

    // When: the map is resolved and checked value views modify the named field.
    let resolved = map.resolve().expect("reusable map must resolve");
    let paths = resolved
        .registers()
        .iter()
        .map(|register| (register.path(), register.offset()))
        .collect::<Vec<_>>();
    let mut little = [0_u8; 2];
    CONTROL
        .view_mut(&mut little)
        .expect("full little-endian view")
        .set_field_by_name("mode", 5)
        .expect("field value fits");
    let big = RegisterDefinition::<FieldId>::new("Big", 4, Endian::Big).with_fields(&FIELDS);
    let mut big_bytes = [0_u8; 4];
    big.view_mut(&mut big_bytes)
        .expect("full big-endian view")
        .set_field_by_name("mode", 3)
        .expect("field value fits");

    // Then: order, paths, exact-start semantics, and endian bytes are literal.
    assert_eq!(
        paths,
        [
            ("Device.primary", 0),
            ("Device.mirror", 0x10),
            ("Device.channel[0].control", 0x20),
            ("Device.channel[1].control", 0x30),
        ]
    );
    assert_eq!(
        resolved.locate(0x20).expect("exact-start hit").path(),
        "Device.channel[0].control"
    );
    assert!(resolved.locate(0x21).is_none());
    assert_eq!(CONTROL.view(&little).expect("read view").value(), Ok(0x50));
    assert_eq!(little, [0x50, 0x00]);
    assert_eq!(big_bytes, [0x00, 0x00, 0x00, 0x30]);
    println!("paths={paths:?}");
    println!("exact=Device.channel[0].control interior=None");
    println!("little={little:02x?} big={big_bytes:02x?}");
}

#[test]
fn checked_views_reject_short_buffers_and_field_overflow_without_mutation() {
    // Given
    let mut short = [0xaa];
    let mut bytes = [0x34, 0x12];
    let before = bytes;

    // When
    let short_result = CONTROL.view_mut(&mut short);
    let overflow = CONTROL
        .view_mut(&mut bytes)
        .expect("full view")
        .set_field_by_name("mode", 8);

    // Then
    assert!(matches!(
        short_result,
        Err(RegisterValueError::BufferTooShort {
            required: 2,
            actual: 1,
        })
    ));
    assert_eq!(
        overflow,
        Err(RegisterValueError::FieldValueOverflow {
            location: SchemaMemberIdentity::new("Control", "mode").at_bit(4),
            bit_width: 3,
            value: 8,
        })
    );
    assert_eq!(bytes, before);
}

#[test]
fn register_precedence_adverse_surface() {
    // Given: a bad referenced definition is hidden behind an earlier expanded-start collision.
    const BAD_FIELDS: [RegisterFieldSpec<FieldId>; 1] = [RegisterFieldSpec::place(
        FieldId::Enable,
        "enable",
        0,
        RegisterFieldDefinition::new(1),
    )];
    const BAD: RegisterDefinition<FieldId> = RegisterDefinition::new("Bad", 1, Endian::Little)
        .with_writable_mask(0x2)
        .with_fields(&BAD_FIELDS);
    const COLLIDING: [RegisterSpec<RegisterId, FieldId>; 1] =
        [RegisterSpec::place(RegisterId::Channel, "bad", 0, BAD)];
    const BAD_BANKS: [RepeatedRegisterBank<RegisterId, FieldId>; 1] =
        [RepeatedRegisterBank::new("bank", 0, 1, 1, &COLLIDING)];
    let map = RegisterMapSpec::named("Adverse", 8, &FIXED[..1], &BAD_BANKS);

    // When
    let result = map.resolve();

    // Then: no partial graph/view exists and the map-phase error wins exactly.
    assert_eq!(
        result,
        Err(RegisterMapError::DuplicateExpandedStart {
            root: "Adverse",
            path: "Adverse.bank[0].bad".into(),
            declaration_index: 1,
            offset: 0,
            first_path: "Adverse.primary".into(),
        })
    );
    println!(
        "error={:?}",
        result.expect_err("collision must reject resolution")
    );
}

#[test]
fn resolved_map_reports_repeated_bank_overlap_with_later_path() {
    // Given
    const OTHER: [RegisterSpec<RegisterId, FieldId>; 1] =
        [RegisterSpec::place(RegisterId::Mirror, "other", 0, CONTROL)];
    const OVERLAPPING: [RepeatedRegisterBank<RegisterId, FieldId>; 2] = [
        RepeatedRegisterBank::new("first", 0x20, 1, 0x10, &MEMBERS),
        RepeatedRegisterBank::new("second", 0x21, 1, 0x10, &OTHER),
    ];

    // When
    let result = RegisterMapSpec::named("Overlap", 0x40, &[], &OVERLAPPING).resolve();

    // Then
    assert_eq!(
        result,
        Err(RegisterMapError::ExpandedOverlap {
            root: "Overlap",
            path: "Overlap.second[0].other".into(),
            declaration_index: 1,
            first_path: "Overlap.first[0].control".into(),
        })
    );
}
