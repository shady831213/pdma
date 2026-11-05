//! Inline storage obeys the same checked geometry as reusable record specifications.
use hw_model_schema::{
    analysis::{AnalysisError, AnalyzedSchema},
    ast::*,
    *,
};

fn invalid_specs() -> Vec<RecordSpec> {
    static OVERLAP: [FieldSpec; 2] = [
        FieldSpec::new(0, "a", 0, ScalarKind::U16),
        FieldSpec::new(1, "b", 1, ScalarKind::U16),
    ];
    static OUTSIDE: [FieldSpec; 1] = [FieldSpec::new(0, "a", 4, ScalarKind::U8)];
    static CHILD: RecordSpec = RecordSpec::named("Child", 4, 1, &[], &[]);
    static PLACEMENTS: [RecordPlacement; 2] = [
        RecordPlacement::new(0, "a", 0, &CHILD),
        RecordPlacement::new(1, "b", 0, &CHILD),
    ];
    vec![
        RecordSpec::named("Storage", 4, 0, &[], &[]),
        RecordSpec::named("Storage", 4, 3, &[], &[]),
        RecordSpec::named("Storage", 4, 1, &OVERLAP, &[]),
        RecordSpec::named("Storage", 4, 1, &OUTSIDE, &[]),
        RecordSpec::named("Storage", 4, 1, &[], &PLACEMENTS),
    ]
}

fn analyze(union: UnionDefinition) -> Result<AnalyzedSchema, AnalysisError> {
    let width = union.storage().size();
    AnalyzedSchema::record(
        "Root",
        RecordDefinition::new(
            RecordSpec::named("Root", width, 1, &[], &[]),
            vec![Field::new(
                "payload",
                0,
                width,
                Endian::Little,
                FieldType::Union(Box::new(union)),
            )],
        ),
    )
}

#[test]
fn inline_union_storage_uses_checked_record_geometry() {
    for spec in invalid_specs() {
        let expected = spec.resolve().unwrap_err();
        let result = analyze(UnionDefinition::new(spec, None, vec![]));
        assert_eq!(
            result.unwrap_err(),
            AnalysisError::Record {
                path: "Storage",
                source: expected,
            }
        );
    }
}

#[test]
fn inline_cases_use_checked_record_geometry() {
    for spec in invalid_specs() {
        let expected = spec.resolve().unwrap_err();
        for selector in [None, Some("tag")] {
            let result = analyze(UnionDefinition::new(
                RecordSpec::new(4, 1, &[]),
                selector,
                vec![Case::inline(
                    "payload_case",
                    selector.map(|_| 1),
                    RecordDefinition::new(spec, vec![]),
                )],
            ));
            assert_eq!(
                result.unwrap_err(),
                AnalysisError::Record {
                    path: "payload_case",
                    source: expected.clone(),
                }
            );
        }
    }
}

#[test]
fn inline_packed_case_and_storage_padding_remain_valid() {
    static PACKED: [FieldSpec; 1] = [FieldSpec::new(0, "word", 1, ScalarKind::U16)];
    let case = RecordDefinition::new(
        RecordSpec::named("Packed", 3, 1, &PACKED, &[]),
        vec![Field::new(
            "word",
            1,
            2,
            Endian::Little,
            FieldType::Scalar {
                definition: FieldDefinition::new(ScalarKind::U16),
                constant: None,
            },
        )],
    );
    for selector in [None, Some("tag")] {
        assert!(
            analyze(UnionDefinition::new(
                RecordSpec::new(4, 1, &[]),
                selector,
                vec![Case::inline("packed", selector.map(|_| 1), case.clone())],
            ))
            .is_ok()
        );
    }
}

fn invalid_reference() -> Definition {
    Definition::opaque::<u32>(RecordSpec::named("Referenced", 4, 0, &[], &[]))
}

#[test]
fn referenced_cases_retain_checked_geometry() {
    let result = analyze(UnionDefinition::new(
        RecordSpec::new(4, 1, &[]),
        None,
        vec![Case::external(Reference::new(
            "external",
            ReferenceKind::Variant,
            invalid_reference,
        ))],
    ));
    assert!(matches!(
        result,
        Err(AnalysisError::Record {
            path: "u32",
            source: RecordGraphError::AlignmentZero {
                root: "Referenced",
                ..
            },
        })
    ));
}
