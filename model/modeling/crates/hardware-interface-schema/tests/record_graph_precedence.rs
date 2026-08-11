//! Nested record validation and resolution precedence coverage.

use hardware_interface_schema::{
    FieldDefinition, FieldSpec, RecordGraphError, RecordPlacement, RecordSpec, ScalarKind,
};

const WORD: FieldDefinition = FieldDefinition::new(ScalarKind::U16);
const CHILD_FIELDS: [FieldSpec; 1] = [FieldSpec::place(1, "value", 0, WORD)];
const CHILD: RecordSpec = RecordSpec::named("child_def", 2, 2, &CHILD_FIELDS, &[]);
const BAD_WIDTH_FIELDS: [FieldSpec; 1] = [FieldSpec::place(4, "bad", 0, WORD.with_width(1))];
const BAD_WIDTH: RecordSpec = RecordSpec::named("bad_width", 2, 2, &BAD_WIDTH_FIELDS, &[]);

const fn recursive_definition() -> &'static RecordSpec {
    &RECURSIVE
}

const RECURSIVE_RECORDS: [RecordPlacement; 1] = [RecordPlacement::deferred(
    8,
    "again",
    0,
    recursive_definition,
)];
const RECURSIVE: RecordSpec = RecordSpec::named("recursive", 2, 1, &[], &RECURSIVE_RECORDS);

fn assert_error(actual: Result<impl Sized, RecordGraphError>, expected: RecordGraphError) {
    assert_eq!(actual.err(), Some(expected));
}

#[test]
fn combined_sibling_phases_when_faults_compound_report_id_then_name() {
    // Given
    const ID_FIELDS: [FieldSpec; 1] = [FieldSpec::place(7, "scalar", 0, WORD)];
    const ID_RECORDS: [RecordPlacement; 1] = [RecordPlacement::new(7, "nested", 2, &CHILD)];
    const ID_BAD: RecordSpec = RecordSpec::named("id_root", 4, 2, &ID_FIELDS, &ID_RECORDS);
    const NAME_FIELDS: [FieldSpec; 1] =
        [FieldSpec::place(1, "same", 0, WORD).with_declaration_index(3)];
    const NAME_RECORDS: [RecordPlacement; 1] =
        [RecordPlacement::new(2, "same", 2, &CHILD).with_declaration_index(1)];
    const NAME_BAD: RecordSpec = RecordSpec::named("name_root", 4, 2, &NAME_FIELDS, &NAME_RECORDS);

    // When
    let cases = [
        (
            ID_BAD.resolve(),
            RecordGraphError::DuplicateSiblingId {
                root: "id_root",
                path: "id_root".into(),
                declaration_index: 1,
                id: 7,
            },
        ),
        (
            NAME_BAD.resolve(),
            RecordGraphError::DuplicateSiblingName {
                root: "name_root",
                path: "name_root".into(),
                declaration_index: 3,
                name: "same",
            },
        ),
    ];

    // Then
    for (actual, expected) in cases {
        assert_error(actual, expected);
    }
}

#[test]
fn local_phase_order_when_faults_compound_is_total() {
    // Given
    const WIDTH_BAD: RecordSpec = RecordSpec::named("width", 1, 0, &BAD_WIDTH_FIELDS, &[]);
    const ALIGN_FIELDS: [FieldSpec; 1] = [FieldSpec::place(1, "tail", 1, WORD)];
    const ALIGN_BAD: RecordSpec = RecordSpec::named("align", 2, 0, &ALIGN_FIELDS, &[]);
    const EXTENT_FIELDS: [FieldSpec; 2] = [
        FieldSpec::place(1, "first", 3, WORD),
        FieldSpec::place(2, "second", 3, WORD),
    ];
    const EXTENT_BAD: RecordSpec = RecordSpec::named("extent", 4, 2, &EXTENT_FIELDS, &[]);
    const OVERLAP_FIELDS: [FieldSpec; 2] = [
        FieldSpec::place(1, "first", 0, WORD),
        FieldSpec::place(2, "second", 1, WORD),
    ];
    const OVERLAP_RECORDS: [RecordPlacement; 1] =
        [RecordPlacement::new(3, "recursive", 4, &RECURSIVE)];
    const OVERLAP_BAD: RecordSpec =
        RecordSpec::named("overlap", 6, 2, &OVERLAP_FIELDS, &OVERLAP_RECORDS);
    let cases = [
        (
            WIDTH_BAD.resolve(),
            RecordGraphError::ScalarWidthMismatch {
                root: "width",
                path: "width.bad".into(),
                declaration_index: 0,
                id: 4,
                scalar: ScalarKind::U16,
                width: 1,
            },
        ),
        (
            ALIGN_BAD.resolve(),
            RecordGraphError::AlignmentZero {
                root: "align",
                path: "align".into(),
                declaration_index: 0,
            },
        ),
        (
            EXTENT_BAD.resolve(),
            RecordGraphError::FieldOutOfBounds {
                root: "extent",
                path: "extent.first".into(),
                declaration_index: 0,
                id: 1,
                offset: 3,
                width: 2,
                record_size: 4,
            },
        ),
        (
            OVERLAP_BAD.resolve(),
            RecordGraphError::FieldOverlap {
                root: "overlap",
                path: "overlap".into(),
                declaration_index: 1,
                first_id: 1,
                second_id: 2,
            },
        ),
    ];

    // When
    for (actual, expected) in cases {
        // Then
        assert_error(actual, expected);
    }
}

#[test]
fn traversal_order_when_bad_child_precedes_later_recursion_reports_child_payload() {
    // Given
    const RECORDS: [RecordPlacement; 2] = [
        RecordPlacement::new(1, "width", 0, &BAD_WIDTH),
        RecordPlacement::new(2, "loop", 2, &RECURSIVE),
    ];
    const BAD: RecordSpec = RecordSpec::named("root", 4, 2, &[], &RECORDS);

    // When
    let actual = BAD.resolve();

    // Then
    assert_error(
        actual,
        RecordGraphError::ScalarWidthMismatch {
            root: "root",
            path: "root.width.bad".into(),
            declaration_index: 0,
            id: 4,
            scalar: ScalarKind::U16,
            width: 1,
        },
    );
}

#[test]
fn recursion_when_resolution_faults_coexist_wins_before_graph_creation() {
    // Given
    const FIELDS: [FieldSpec; 1] = [FieldSpec::place(1, "loop.again", 0, WORD)];
    const RECORDS: [RecordPlacement; 1] = [RecordPlacement::new(2, "loop", 3, &RECURSIVE)];
    const BAD: RecordSpec = RecordSpec::named("adverse", 4, 1, &FIELDS, &RECORDS);

    // When
    let actual = BAD.resolve();

    // Then
    println!(
        "error=RecursiveRecord root=adverse path=adverse.loop.again index=0 definition=recursive graph=none"
    );
    assert_error(
        actual,
        RecordGraphError::RecursiveRecord {
            root: "adverse",
            path: "adverse.loop.again".into(),
            declaration_index: 0,
            definition: "recursive",
        },
    );
}

#[test]
fn resolution_phases_when_faults_compound_report_range_overlap_then_path() {
    // Given
    const RANGE_FIELDS: [FieldSpec; 1] = [FieldSpec::place(1, "child.value", 0, WORD)];
    const RANGE_RECORDS: [RecordPlacement; 2] = [
        RecordPlacement::new(2, "child", 3, &CHILD),
        RecordPlacement::new(3, "other", 2, &CHILD),
    ];
    const RANGE_BAD: RecordSpec = RecordSpec::named("range", 4, 2, &RANGE_FIELDS, &RANGE_RECORDS);
    const OVERLAP_RECORDS: [RecordPlacement; 2] = [
        RecordPlacement::new(1, "first", 0, &CHILD),
        RecordPlacement::new(2, "second", 1, &CHILD),
    ];
    const OVERLAP_BAD: RecordSpec = RecordSpec::named("nested", 4, 2, &[], &OVERLAP_RECORDS);
    const PATH_FIELDS: [FieldSpec; 1] = [FieldSpec::place(1, "child.value", 2, WORD)];
    const PATH_RECORDS: [RecordPlacement; 1] = [RecordPlacement::new(2, "child", 0, &CHILD)];
    const PATH_BAD: RecordSpec = RecordSpec::named("path", 4, 2, &PATH_FIELDS, &PATH_RECORDS);
    let cases = [
        (
            RANGE_BAD.resolve(),
            RecordGraphError::PlacementOutOfBounds {
                root: "range",
                path: "range.child".into(),
                declaration_index: 1,
                offset: 3,
                size: 2,
                record_size: 4,
            },
        ),
        (
            OVERLAP_BAD.resolve(),
            RecordGraphError::MemberOverlap {
                root: "nested",
                path: "nested".into(),
                declaration_index: 1,
                first_path: "nested.first".into(),
                second_path: "nested.second".into(),
            },
        ),
        (
            PATH_BAD.resolve(),
            RecordGraphError::ResolvedPathCollision {
                root: "path",
                path: "path.child.value".into(),
                declaration_index: 0,
                first_declaration_index: 0,
            },
        ),
    ];

    // When
    for (actual, expected) in cases {
        // Then
        assert_error(actual, expected);
    }
}

#[test]
fn repeated_offset_overflow_reports_instance_path_and_index() {
    // Given
    const RECORDS: [RecordPlacement; 1] = [RecordPlacement::repeated(
        1,
        "item",
        0,
        2,
        usize::MAX,
        &CHILD,
    )];
    const BAD: RecordSpec = RecordSpec::named("overflow", usize::MAX, 1, &[], &RECORDS);

    // When
    let actual = BAD.resolve();

    // Then
    assert_error(
        actual,
        RecordGraphError::PlacementOffsetOverflow {
            root: "overflow",
            path: "overflow.item[1]".into(),
            declaration_index: 0,
            offset: 0,
            instance: 1,
            stride: usize::MAX,
        },
    );
}
