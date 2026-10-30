//! Borrowed scalar traversal preserves resolved placement semantics.

use hw_model_schema::{Documentation, Endian, FieldSpec, RecordPlacement, RecordSpec, ScalarKind};

#[test]
fn scalar_members_keep_repeated_offsets_endian_and_effective_documentation() {
    // Given a scalar followed by repeated nested records with placement documentation.
    const FIELDS: [FieldSpec; 1] = [FieldSpec::new(1, "word", 0, ScalarKind::U16)
        .with_endian(Endian::Big)
        .with_documentation(Documentation {
            short_description: Some("placed word"),
            description: None,
        })];
    const CHILD: RecordSpec = RecordSpec::named("Child", 2, 2, &FIELDS, &[]);
    const TAG: [FieldSpec; 1] = [FieldSpec::new(1, "tag", 0, ScalarKind::U8)];
    const RECORDS: [RecordPlacement; 1] = [RecordPlacement::repeated(2, "items", 4, 2, 4, &CHILD)];
    let resolved = RecordSpec::named("Root", 12, 4, &TAG, &RECORDS)
        .resolve()
        .expect("valid repeated record");

    // When a downstream consumer visits scalar members only.
    let rows: Vec<_> = resolved
        .scalar_members()
        .map(|(member, definition)| {
            (
                member.path(),
                member.absolute_offset(),
                definition.width(),
                definition.endian(),
                member.documentation().short_description,
            )
        })
        .collect();

    // Then containers are omitted without changing scalar order or placement metadata.
    assert_eq!(
        rows,
        [
            ("Root.tag", 0, 1, Endian::Little, None),
            ("Root.items[0].word", 4, 2, Endian::Big, Some("placed word")),
            ("Root.items[1].word", 8, 2, Endian::Big, Some("placed word")),
        ]
    );
}

#[test]
fn scalar_members_of_empty_record_are_empty() {
    // Given an empty record and its resolved representation.
    let resolved = RecordSpec::new(0, 1, &[]).resolve().expect("empty record");

    // When visiting only scalar members, then no row is produced.
    assert_eq!(resolved.scalar_members().count(), 0);
}
