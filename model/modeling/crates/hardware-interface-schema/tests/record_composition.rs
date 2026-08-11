//! Record composition and checked-view integration coverage.

use hardware_interface_schema::{
    Endian, FieldDefinition, FieldSpec, RecordPlacement, RecordSpec, ResolvedMemberKind,
    ScalarKind, SchemaCodecError, write_scalar,
};

const WORD: FieldDefinition = FieldDefinition::new(ScalarKind::U16);
const BIG_WORD: FieldDefinition = WORD.with_endian(Endian::Big);
const CHILD_FIELDS: [FieldSpec; 2] = [
    FieldSpec::place(1, "low", 0, WORD),
    FieldSpec::place(2, "high", 2, BIG_WORD),
];
const CHILD: RecordSpec = RecordSpec::named("pair", 4, 2, &CHILD_FIELDS, &[]);
const ROOT_FIELDS: [FieldSpec; 1] = [FieldSpec::place(
    1,
    "tag",
    0,
    FieldDefinition::new(ScalarKind::U8),
)];
const ROOT_RECORDS: [RecordPlacement; 2] = [
    RecordPlacement::new(2, "left", 4, &CHILD),
    RecordPlacement::new(3, "right", 8, &CHILD),
];
const ROOT: RecordSpec = RecordSpec::named("packet", 12, 4, &ROOT_FIELDS, &ROOT_RECORDS);

#[test]
fn field_definition_when_placed_twice_retains_distinct_identity_and_offset() {
    // Given
    const FIELDS: [FieldSpec; 2] = [
        FieldSpec::place(4, "first", 0, WORD),
        FieldSpec::place(5, "second", 2, WORD),
    ];
    const RECORD: RecordSpec = RecordSpec::named("words", 4, 2, &FIELDS, &[]);

    // When
    let resolved = RECORD.resolve().expect("reused scalar definition is valid");

    // Then
    let paths = resolved
        .members()
        .iter()
        .map(|member| (member.path(), member.absolute_offset()))
        .collect::<Vec<_>>();
    assert_eq!(paths, [("words.first", 0), ("words.second", 2)]);
    assert_eq!(FIELDS[0].definition(), FIELDS[1].definition());
}

#[test]
fn record_definition_when_placed_twice_resolves_depth_first_at_each_base() {
    // Given
    let expected = [
        ("packet.tag", 0),
        ("packet.left", 4),
        ("packet.left.low", 4),
        ("packet.left.high", 6),
        ("packet.right", 8),
        ("packet.right.low", 8),
        ("packet.right.high", 10),
    ];

    // When
    let resolved = ROOT.resolve().expect("nested placements are valid");

    // Then
    let actual = resolved
        .members()
        .iter()
        .map(|member| (member.path(), member.absolute_offset()))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    let left = resolved.members().get(1).expect("left placement exists");
    assert!(matches!(
        left.kind(),
        ResolvedMemberKind::Record { instance: 0, .. }
    ));
}

#[test]
fn repeated_record_when_resolved_in_declaration_order_has_indexed_paths() {
    // Given
    const RECORDS: [RecordPlacement; 1] =
        [RecordPlacement::repeated(9, "channel", 0, 2, 8, &CHILD)];
    const BANK: RecordSpec = RecordSpec::named("bank", 12, 4, &[], &RECORDS);

    // When
    let resolved = BANK.resolve().expect("repeated child is valid");

    // Then
    let actual = resolved
        .members()
        .iter()
        .map(|member| (member.path(), member.absolute_offset()))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            ("bank.channel[0]", 0),
            ("bank.channel[0].low", 0),
            ("bank.channel[0].high", 2),
            ("bank.channel[1]", 8),
            ("bank.channel[1].low", 8),
            ("bank.channel[1].high", 10),
        ]
    );
}

#[test]
fn distinct_definitions_when_names_match_do_not_imply_recursion() {
    // Given
    const INNER_FIELDS: [FieldSpec; 1] = [FieldSpec::place(1, "low", 0, WORD)];
    const INNER: RecordSpec = RecordSpec::named("same", 2, 2, &INNER_FIELDS, &[]);
    const RECORDS: [RecordPlacement; 1] = [RecordPlacement::new(9, "inner", 0, &INNER)];
    const OUTER: RecordSpec = RecordSpec::named("same", 2, 2, &[], &RECORDS);

    // When
    let resolved = OUTER.resolve().expect("distinct definition resolves");

    // Then
    let paths = resolved
        .members()
        .iter()
        .map(hardware_interface_schema::ResolvedMember::path)
        .collect::<Vec<_>>();
    assert_eq!(paths, ["same.inner", "same.inner.low"]);
}

#[test]
fn checked_views_when_record_or_member_buffer_is_short_return_typed_error() {
    // Given
    let resolved = ROOT.resolve().expect("fixture resolves");
    let right = resolved
        .members()
        .iter()
        .find(|member| member.path() == "packet.right")
        .expect("right placement exists");

    // When
    let record_error = resolved.view(&[0_u8; 11]);
    let member_error = right.view(&[0_u8; 11]);

    // Then
    let short = SchemaCodecError::BufferTooShort {
        required: 12,
        actual: 11,
    };
    assert_eq!(record_error, Err(short));
    assert_eq!(member_error, Err(short));
}

#[test]
fn nested_record_surface() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let resolved = ROOT.resolve()?;
    let mut bytes = [0_u8; 12];

    // When
    for (path, value) in [
        ("packet.left.low", 0x1122_u16),
        ("packet.left.high", 0x3344_u16),
        ("packet.right.low", 0x5566_u16),
        ("packet.right.high", 0x7788_u16),
    ] {
        let member = resolved
            .members()
            .iter()
            .find(|member| member.path() == path)
            .ok_or("resolved path missing")?;
        let endian = match member.kind() {
            ResolvedMemberKind::Field { definition, .. } => definition.endian(),
            ResolvedMemberKind::Record { .. } => return Err("expected scalar member".into()),
        };
        write_scalar(member.view_mut(&mut bytes)?, endian, value)?;
    }

    // Then
    let paths = resolved
        .members()
        .iter()
        .map(|member| format!("{}@{}", member.path(), member.absolute_offset()))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        bytes,
        [0, 0, 0, 0, 0x22, 0x11, 0x33, 0x44, 0x66, 0x55, 0x77, 0x88]
    );
    println!("paths={paths} bytes=00 00 00 00 22 11 33 44 66 55 77 88");
    Ok(())
}
