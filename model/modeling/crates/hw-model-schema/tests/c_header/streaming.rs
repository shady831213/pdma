use hw_model_schema::{
    analysis::{AnalyzedSchema, Pipeline},
    export::{
        ExportError,
        backends::c_header::{CHeaderExportError, CRecordAnalysisPass, CRecordBackend},
    },
};
use std::io::{self, Write};

struct StopAfter {
    remaining: usize,
    written: Vec<u8>,
    failures: usize,
}
impl Write for StopAfter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            self.failures += 1;
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "consumer stopped",
            ));
        }
        let count = bytes.len().min(self.remaining);
        self.written.extend_from_slice(
            bytes
                .get(..count)
                .ok_or_else(|| io::Error::other("invalid byte count"))?,
        );
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn record_stream_reports_partial_write_failure() -> super::support::TestResult {
    let schema = AnalyzedSchema::opaque("packet", super::support::RECORD)?;
    let prepared = Pipeline::new((&schema, ())).run(CRecordAnalysisPass)?;
    let expected =
        super::support::render_record("packet", "packet", &super::support::RECORD)?.bytes;
    for remaining in 0..expected.len() {
        let mut writer = StopAfter {
            remaining,
            written: Vec::new(),
            failures: 0,
        };
        let error = prepared
            .generate(CRecordBackend::new, &mut writer)
            .unwrap_err();
        let ExportError::Render(CHeaderExportError::Io(cause)) = error else {
            panic!("wrong error: {error}");
        };
        assert_eq!(cause.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(cause.to_string(), "consumer stopped");
        assert_eq!(
            writer.written,
            expected.get(..remaining).ok_or("missing expected prefix")?
        );
        assert_eq!(
            writer.failures, 1,
            "writes continued after failure at byte {remaining}"
        );
    }
    Ok(())
}

#[test]
fn register_stream_stops_before_successor_output() -> super::support::TestResult {
    use hw_model_schema::{
        RecordSpec,
        analysis::Bindings,
        ast::{Definition, SchemaSet},
        export::{backends::c_header::CRegisterBackend, generate},
    };
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<u8>(RecordSpec::named(
        "First",
        1,
        1,
        &[],
        &[],
    )));
    set.add_definition(Definition::opaque::<u16>(RecordSpec::named(
        "Second",
        2,
        2,
        &[],
        &[],
    )));
    let graph = set.resolve()?;
    let schema = AnalyzedSchema::from_graph(&graph, &Bindings::new())?;
    let mut output = StopAfter {
        remaining: 13,
        written: Vec::new(),
        failures: 0,
    };
    let error = generate(&schema, CRegisterBackend::new(), &mut output).unwrap_err();
    let source = std::error::Error::source(&error).ok_or("missing render source")?;
    let io_source = source
        .source()
        .ok_or("missing IO source")?
        .downcast_ref::<io::Error>()
        .ok_or("wrong IO source type")?;
    assert_eq!(io_source.kind(), io::ErrorKind::BrokenPipe);
    let ExportError::Render(CHeaderExportError::Io(cause)) = error else {
        panic!("wrong error: {error}");
    };
    assert_eq!(cause.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(cause.to_string(), "consumer stopped");
    assert_eq!(output.written.len(), 13);
    assert!(output.written.starts_with(b"#define C_"));
    assert_eq!(output.failures, 1);
    Ok(())
}

#[test]
fn prepared_roots_retain_independent_c_prefixes() -> super::support::TestResult {
    use hw_model_schema::export::{backends::c_header::CRegisterBackend, prepare};
    use hw_model_schema::{
        RecordSpec,
        analysis::Bindings,
        ast::{Definition, SchemaSet},
    };
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<u8>(RecordSpec::named(
        "First",
        1,
        1,
        &[],
        &[],
    )));
    set.add_definition(Definition::opaque::<u16>(RecordSpec::named(
        "Second",
        2,
        2,
        &[],
        &[],
    )));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let analysis = Pipeline::new((&schema, ())).run(CRecordAnalysisPass)?;
    let mut record = CRecordBackend::new(&analysis.input().1);
    let mut register = CRegisterBackend::new();
    for root in prepare(&schema, &mut record)? {
        let expected = format!(
            "#define C_{}_SIZE ",
            root.metadata().logical_name.to_ascii_uppercase()
        );
        let mut bytes = Vec::new();
        root.emit(&mut record, &mut bytes)?;
        assert!(std::str::from_utf8(&bytes)?.starts_with(&expected));
    }
    for root in prepare(&schema, &mut register)? {
        let expected = format!(
            "#define C_{}_SIZE ",
            root.metadata().logical_name.to_ascii_uppercase()
        );
        let mut bytes = Vec::new();
        root.emit(&mut register, &mut bytes)?;
        assert!(std::str::from_utf8(&bytes)?.starts_with(&expected));
    }
    Ok(())
}

#[test]
fn root_alias_collision_prevents_all_c_output() -> super::support::TestResult {
    use hw_model_schema::export::{backends::c_header::CRegisterBackend, generate};
    use hw_model_schema::{
        RecordSpec,
        analysis::Bindings,
        ast::{Definition, SchemaSet},
    };
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<u8>(RecordSpec::named(
        "First",
        1,
        1,
        &[],
        &[],
    )));
    set.add_definition(Definition::opaque::<u16>(RecordSpec::named(
        "Second",
        2,
        2,
        &[],
        &[],
    )));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new())?;
    let mut backend = CRegisterBackend::new();
    for unit in schema.units() {
        backend.set_name(unit.id.clone(), "shared");
    }
    let mut bytes = Vec::new();
    let error = generate(&schema, backend, &mut bytes).unwrap_err();
    assert!(matches!(
        error,
        ExportError::Render(CHeaderExportError::NameCollision {
            first_index: 0,
            second_index: 1,
            ..
        })
    ));
    assert!(bytes.is_empty());
    Ok(())
}
