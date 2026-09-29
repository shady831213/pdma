//! Downstream-style checks for the format-neutral export extension boundary.

use core::convert::Infallible;

use hardware_interface_schema::{
    RecordSpec, ResolvedRecord,
    export::{Artifact, ArtifactSink, ExportError, Exporter, InMemoryArtifactSink},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalExportError {
    ExporterRejected,
    SinkRejected,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct LocalCollectingSink {
    artifacts: Vec<Artifact>,
}

impl ArtifactSink for LocalCollectingSink {
    type Error = LocalExportError;

    fn emit(&mut self, artifact: Artifact) -> Result<(), Self::Error> {
        self.artifacts.push(artifact);
        Ok(())
    }
}

#[derive(Default)]
struct RejectingSink {
    attempted: usize,
}

impl ArtifactSink for RejectingSink {
    type Error = LocalExportError;

    fn emit(&mut self, _artifact: Artifact) -> Result<(), Self::Error> {
        self.attempted += 1;
        Err(LocalExportError::SinkRejected)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CountingSink {
    emitted: usize,
}

impl ArtifactSink for CountingSink {
    type Error = LocalExportError;

    fn emit(&mut self, _artifact: Artifact) -> Result<(), Self::Error> {
        self.emitted += 1;
        Ok(())
    }
}

struct LocalExporter;

impl Exporter<ResolvedRecord> for LocalExporter {
    type Error = LocalExportError;

    fn export<Sink>(
        &self,
        _schema: &ResolvedRecord,
        sink: &mut Sink,
    ) -> Result<(), ExportError<Self::Error, Sink::Error>>
    where
        Sink: ArtifactSink,
    {
        let logical_name = String::from("record-layout.bin");
        let media_type = String::from("application/x-local-schema");
        let contents = vec![0, 255, 7];
        sink.emit(Artifact {
            logical_name,
            media_type,
            contents,
        })
        .map_err(ExportError::Sink)?;
        sink.emit(Artifact {
            logical_name: String::from("record-layout.note"),
            media_type: String::from("text/plain"),
            contents: Vec::new(),
        })
        .map_err(ExportError::Sink)
    }
}

struct RejectingExporter;

impl Exporter<ResolvedRecord> for RejectingExporter {
    type Error = LocalExportError;

    fn export<Sink>(
        &self,
        _schema: &ResolvedRecord,
        _sink: &mut Sink,
    ) -> Result<(), ExportError<Self::Error, Sink::Error>>
    where
        Sink: ArtifactSink,
    {
        Err(ExportError::Render(LocalExportError::ExporterRejected))
    }
}

struct InfallibleLocalExporter;

impl Exporter<ResolvedRecord> for InfallibleLocalExporter {
    type Error = Infallible;

    fn export<Sink>(
        &self,
        _schema: &ResolvedRecord,
        sink: &mut Sink,
    ) -> Result<(), ExportError<Self::Error, Sink::Error>>
    where
        Sink: ArtifactSink,
    {
        sink.emit(Artifact {
            logical_name: String::from("odd-content"),
            media_type: String::from("application/x-local-schema"),
            contents: vec![0, 255, 7],
        })
        .map_err(ExportError::Sink)
    }
}

const fn record_schema() -> RecordSpec {
    const FIELDS: [hardware_interface_schema::FieldSpec; 0] = [];
    RecordSpec::new(0, 1, &FIELDS)
}

#[test]
fn downstream_local_exporter_and_sink_preserve_owned_artifacts_in_emission_order() {
    // Given
    let schema = record_schema().resolve().expect("fixture must resolve");
    let exporter = LocalExporter;
    let mut sink = LocalCollectingSink::default();

    // When
    let result = exporter.export(&schema, &mut sink);

    // Then
    assert_eq!(result, Ok(()));
    assert_eq!(
        sink.artifacts,
        vec![
            Artifact {
                logical_name: String::from("record-layout.bin"),
                media_type: String::from("application/x-local-schema"),
                contents: vec![0, 255, 7],
            },
            Artifact {
                logical_name: String::from("record-layout.note"),
                media_type: String::from("text/plain"),
                contents: Vec::new(),
            },
        ]
    );
}

#[test]
fn in_memory_sink_preserves_owned_odd_contents_after_exporter_locals_drop() {
    // Given
    let schema = record_schema().resolve().expect("fixture must resolve");
    let exporter = InfallibleLocalExporter;
    let mut sink = InMemoryArtifactSink::new();

    // When
    let result = exporter.export(&schema, &mut sink);

    // Then
    assert_eq!(result, Ok(()));
    assert_eq!(
        sink.artifacts(),
        &[Artifact {
            logical_name: String::from("odd-content"),
            media_type: String::from("application/x-local-schema"),
            contents: vec![0, 255, 7],
        }]
    );
}

#[test]
fn sink_rejection_reaches_the_caller_as_the_exact_typed_error() {
    // Given
    let schema = record_schema().resolve().expect("fixture must resolve");
    let exporter = LocalExporter;
    let mut sink = RejectingSink::default();

    // When
    let result = exporter.export(&schema, &mut sink);

    // Then
    assert_eq!(
        result,
        Err(ExportError::Sink(LocalExportError::SinkRejected))
    );
    assert_eq!(sink.attempted, 1);
}

#[test]
fn exporter_rejection_does_not_invoke_the_sink() {
    // Given
    let schema = record_schema().resolve().expect("fixture must resolve");
    let exporter = RejectingExporter;
    let mut sink = CountingSink::default();

    // When
    let result = exporter.export(&schema, &mut sink);

    // Then
    assert_eq!(
        result,
        Err(ExportError::Render(LocalExportError::ExporterRejected))
    );
    assert_eq!(sink.emitted, 0);
}
