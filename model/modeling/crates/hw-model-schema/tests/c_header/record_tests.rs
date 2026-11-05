use hw_model_schema::analysis::AnalyzedSchema;
use hw_model_schema::{
    FieldSpec, RecordPlacement, RecordSpec, ScalarKind,
    export::ExportError,
    export::backends::c_header::{CHeaderExportError, CRecordBackend},
};

use super::support::{
    NESTED_C_MAIN, PREFIX_C_MAIN, RECORD, TestResult, compile_fragments, render_record,
};

const RECORD_GOLDEN: &[u8] = include_bytes!("../golden/c_header_record.h");

const CHILD_FIELDS: [FieldSpec; 1] = [FieldSpec::new(1, "value", 0, ScalarKind::U16)];
const CHILD: RecordSpec = RecordSpec::named("Child", 2, 2, &CHILD_FIELDS, &[]);
const NESTED: [RecordPlacement; 1] = [RecordPlacement::repeated(1, "items", 4, 2, 4, &CHILD)];
const NESTED_RECORD: RecordSpec = RecordSpec::named("Packet", 12, 4, &[], &NESTED);
const LONG_COLLISION_FIRST: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaax";
const LONG_COLLISION_SECOND: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaay";
const LONG_COLLISION_FIELDS: [FieldSpec; 2] = [
    FieldSpec::new(1, LONG_COLLISION_FIRST, 0, ScalarKind::U8),
    FieldSpec::new(2, LONG_COLLISION_SECOND, 1, ScalarKind::U8),
];
const LONG_COLLISION_RECORD: RecordSpec =
    RecordSpec::named("CollisionRoot", 2, 1, &LONG_COLLISION_FIELDS, &[]);

#[test]
fn c_header_record_is_exact_and_deterministic_when_rendered_twice() -> TestResult {
    // Given: a non-PDMA record with gaps and both byte orders.
    let exporter = ("packet.fragment.h", "Lab-9 packet");

    // When: the record is exported twice through the format-neutral interface.
    let resolved = RECORD;
    let first = render_record(exporter.0, exporter.1, &resolved)?;
    let second = render_record(exporter.0, exporter.1, &resolved)?;

    // Then: both owned UTF-8 artifacts exactly match the reviewed golden fragment.
    assert_eq!(first, second);
    assert_eq!(first.metadata.logical_name, "packet.fragment.h");
    assert_eq!(first.metadata.media_type, "text/x-c; charset=utf-8");
    assert_eq!(first.bytes, RECORD_GOLDEN);
    Ok(())
}

#[test]
fn c_header_nested_record_uses_resolved_paths_and_absolute_offsets() -> TestResult {
    // Given: one reusable record definition placed twice under a named root.
    let resolved = NESTED_RECORD;
    let exporter = ("nested", "nested");

    // When: the resolved graph is rendered and compiled as strict C11.
    let artifact = render_record(exporter.0, exporter.1, &resolved)?;
    compile_fragments("nested", &[&artifact.bytes], NESTED_C_MAIN)?;

    // Then: repeated paths name distinct fields at their absolute offsets.
    let fragment = std::str::from_utf8(&artifact.bytes)?;
    assert!(fragment.contains("C_NESTED_C_ITEMSQX5B0QX5DQX2EVALUE_OFFSET UINT64_C(4)"));
    assert!(fragment.contains("C_NESTED_C_ITEMSQX5B1QX5DQX2EVALUE_OFFSET UINT64_C(8)"));
    Ok(())
}

#[derive(Debug, Default)]
struct RejectingSink {
    attempted: usize,
}

impl std::io::Write for RejectingSink {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        self.attempted += 1;
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "rejected 17",
        ))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct CountingSink {
    attempted: usize,
}
impl std::io::Write for CountingSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.attempted += 1;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn c_header_resolved_collision_is_typed_before_bytes_are_written() -> TestResult {
    // Given: distinct resolved paths whose C11-significant prefixes are identical.
    let resolved = LONG_COLLISION_RECORD;
    let exporter = ("collision", "collision");
    let mut sink = CountingSink::default();

    // When: the real public exporter projects the resolved graph into its C namespace.
    let schema = AnalyzedSchema::opaque(exporter.0, resolved)?;
    let id = schema.view(exporter.0).ok_or("missing view")?.id.clone();
    let result = hw_model_schema::analysis::Pipeline::new((&schema, ()))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?
        .generate(
            |data| CRecordBackend::new(data).with_name(id, exporter.1.as_ref()),
            &mut sink,
        );

    // Then: collision validation retains both paths before writing bytes.
    let expected = CHeaderExportError::NameCollision {
        first_path: format!("CollisionRoot.{LONG_COLLISION_FIRST}"),
        second_path: format!("CollisionRoot.{LONG_COLLISION_SECOND}"),
        first_index: 0,
        second_index: 1,
    };
    assert_eq!(
        std::error::Error::source(result.as_ref().unwrap_err())
            .unwrap()
            .to_string(),
        expected.to_string()
    );
    let ExportError::Render(CHeaderExportError::NameCollision {
        first_path,
        second_path,
        first_index,
        second_index,
    }) = result.unwrap_err()
    else {
        panic!("expected collision");
    };
    assert_eq!(first_path, format!("CollisionRoot.{LONG_COLLISION_FIRST}"));
    assert_eq!(
        second_path,
        format!("CollisionRoot.{LONG_COLLISION_SECOND}")
    );
    assert_eq!((first_index, second_index), (0, 1));
    assert_eq!(sink.attempted, 0);
    println!("collision={expected:?};sink_attempts={}", sink.attempted);
    Ok(())
}

#[test]
fn c_header_sink_rejection_is_typed_and_emits_no_partial_artifact() -> TestResult {
    // Given: a valid resolved record and a sink that rejects before accepting bytes.
    let resolved = RECORD;
    let exporter = ("rejected", "rejected");
    let mut sink = RejectingSink::default();

    // When: the resolved graph is exported through the rejecting sink.
    let schema = AnalyzedSchema::opaque(exporter.0, resolved)?;
    let id = schema.view(exporter.0).ok_or("missing view")?.id.clone();
    let result = hw_model_schema::analysis::Pipeline::new((&schema, ()))
        .run(hw_model_schema::export::backends::c_header::CRecordAnalysisPass)?
        .generate(
            |data| CRecordBackend::new(data).with_name(id, exporter.1.as_ref()),
            &mut sink,
        );

    // Then: the exact sink variant escapes and no artifact was accepted.
    assert!(
        matches!(&result, Err(ExportError::Render(CHeaderExportError::Io(error))) if error.kind() == std::io::ErrorKind::PermissionDenied && error.to_string() == "rejected 17")
    );
    assert_eq!(sink.attempted, 1);
    println!(
        "sink_result={result:?};attempted={};accepted={}",
        sink.attempted, 0
    );
    Ok(())
}

#[test]
fn c_header_symbol_prefix_is_portable_when_caller_name_contains_special_characters() -> TestResult {
    // Given: a caller prefix beginning with a digit and containing repeated punctuation.
    let exporter = ("special", "9 weird--name!");

    // When: the record crosses the generic C naming boundary.
    let artifact = render_record(exporter.0, exporter.1, &RECORD)?;

    // Then: the fragment uses the deterministic sanitized C identifier.
    assert!(
        artifact
            .bytes
            .starts_with(b"#define C_9QX20WEIRDQX2DQX2DNAMEQX21_SIZE ")
    );
    Ok(())
}

#[test]
fn c_header_keeps_independent_prefixes_collision_free_in_one_translation_unit() -> TestResult {
    // Given: separately rendered fragments whose punctuation and case used to normalize equally.
    let punctuation = ("punctuation", "a-b");
    let underscore = ("underscore", "a_b");
    let uppercase = ("uppercase", "A_B");
    let resolved = RECORD;
    let first = render_record(punctuation.0, punctuation.1, &resolved)?;
    let second = render_record(underscore.0, underscore.1, &resolved)?;
    let third = render_record(uppercase.0, uppercase.1, &resolved)?;
    assert_eq!(
        first,
        render_record(punctuation.0, punctuation.1, &resolved)?
    );

    // When: all independently rendered fragments share one strict C11 translation unit.
    compile_fragments(
        "prefix-collision",
        &[&first.bytes, &second.bytes, &third.bytes],
        PREFIX_C_MAIN,
    )?;

    // Then: each caller prefix owns a distinct deterministic C namespace.
    assert_ne!(first.bytes, second.bytes);
    assert_ne!(second.bytes, third.bytes);
    Ok(())
}
