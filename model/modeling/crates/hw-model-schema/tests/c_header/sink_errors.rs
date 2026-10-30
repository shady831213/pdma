use hw_model_schema::ast::analysis::{Analysis, Bindings};
use hw_model_schema::{
    ast::{Definition, SchemaSet},
    export::backends::c_header::CHeaderBackend,
    export::{Artifact, ArtifactSink, ExportError, generate},
    register::RegisterSpec,
    register_map::RegisterMapSpec,
};
use std::{error::Error, fmt, io};

#[derive(Debug)]
struct StorageFailure {
    destination: String,
    cause: io::Error,
}

impl fmt::Display for StorageFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.destination, self.cause)
    }
}

impl Error for StorageFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

struct RejectingStorage {
    attempts: usize,
}

impl ArtifactSink for RejectingStorage {
    type Error = StorageFailure;

    fn emit(&mut self, _: Artifact) -> Result<(), Self::Error> {
        self.attempts += 1;
        Err(StorageFailure {
            destination: String::from("device/headers/output.h"),
            cause: io::Error::new(io::ErrorKind::PermissionDenied, "storage is read only"),
        })
    }
}

fn retains_storage_error(schema: &Analysis) -> super::support::TestResult {
    let mut sink = RejectingStorage { attempts: 0 };
    let error = generate(schema, CHeaderBackend::new(), &mut sink)
        .err()
        .ok_or("sink unexpectedly accepted")?;
    assert_eq!(
        error.source().ok_or("missing export source")?.to_string(),
        "device/headers/output.h: storage is read only"
    );
    let ExportError::Sink(storage) = error else {
        return Err("renderer must retain the independent storage error".into());
    };
    assert_eq!(storage.destination, "device/headers/output.h");
    assert_eq!(storage.cause.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        storage
            .source()
            .ok_or("missing storage source")?
            .to_string(),
        "storage is read only"
    );
    assert_eq!(sink.attempts, 1);
    Ok(())
}

#[test]
fn record_export_preserves_independent_non_clone_sink_error() -> super::support::TestResult {
    // Given: a resolved record and an exporter unrelated to the storage error type.
    let schema = super::support::RECORD;
    // When/Then: rejecting storage receives one artifact and returns its exact typed cause.
    retains_storage_error(&Analysis::opaque("record", schema)?)
}

#[test]
fn register_map_export_preserves_independent_non_clone_sink_error() -> super::support::TestResult {
    // Given: a resolved register map and the same independent storage error type.
    const REGISTERS: [RegisterSpec<u8>; 1] = [RegisterSpec::new(0, "status", 0, 4)];
    let schema = RegisterMapSpec::new(4, &REGISTERS, &[]).resolve().unwrap();
    // When/Then: rejecting storage receives one artifact and returns its exact typed cause.
    retains_storage_error(&Analysis::register_map("map", &schema))
}

#[test]
fn graph_export_preserves_independent_non_clone_sink_error() -> super::support::TestResult {
    // Given: a selected record graph uses the same independent storage error type.
    let mut schemas = SchemaSet::new();
    schemas.add_definition(Definition::opaque::<()>(super::support::RECORD));
    let graph = schemas.resolve()?;

    // When/Then: graph export returns the original sink failure after one attempt.
    retains_storage_error(&Analysis::from_graph(&graph, &Bindings::new())?)
}
