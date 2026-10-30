//! Downstream-style checks for the format-neutral export extension boundary.

use core::convert::Infallible;

use hw_model_schema::{
    RecordSpec, ResolvedRecord,
    export::{
        Artifact, ArtifactSink, DefinitionHeader, ExportError, InMemoryArtifactSink,
        PreparedSchema, Renderer, generate,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalExportError {
    RendererRejected,
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

#[derive(Default)]
struct LocalRenderer {
    record_size: Option<usize>,
}
impl Renderer for LocalRenderer {
    type Error = LocalExportError;
    fn begin_record(&mut self, record: &ResolvedRecord) -> Result<(), Self::Error> {
        self.record_size = Some(record.size());
        Ok(())
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        let size = self.record_size.ok_or(LocalExportError::RendererRejected)?;
        let size = u8::try_from(size).map_err(|_| LocalExportError::RendererRejected)?;
        Ok(vec![
            Artifact {
                logical_name: "record-layout.bin".into(),
                media_type: "application/x-local-schema".into(),
                contents: vec![size, 255, 7],
            },
            Artifact {
                logical_name: "record-layout.note".into(),
                media_type: "text/plain".into(),
                contents: Vec::new(),
            },
        ])
    }
}
struct RejectingRenderer;
impl Renderer for RejectingRenderer {
    type Error = LocalExportError;
    fn begin_definition(&mut self, _definition: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        Err(LocalExportError::RendererRejected)
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(Vec::new())
    }
}
struct InfallibleLocalRenderer;
impl Renderer for InfallibleLocalRenderer {
    type Error = Infallible;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "odd-content".into(),
            media_type: "application/x-local-schema".into(),
            contents: vec![0, 255, 7],
        }])
    }
}

const fn record_schema() -> RecordSpec {
    const FIELDS: [hw_model_schema::FieldSpec; 0] = [];
    RecordSpec::new(0, 1, &FIELDS)
}

#[test]
fn downstream_local_renderer_and_sink_preserve_owned_artifacts_in_emission_order() {
    // Given
    let record = record_schema().resolve().expect("fixture must resolve");
    let schema = PreparedSchema::record("record", &record);
    let renderer = LocalRenderer::default();
    let mut sink = LocalCollectingSink::default();

    // When
    let result = generate(&schema, renderer, &mut sink);

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
fn in_memory_sink_preserves_owned_odd_contents_after_renderer_locals_drop() {
    // Given
    let record = record_schema().resolve().expect("fixture must resolve");
    let schema = PreparedSchema::record("record", &record);
    let renderer = InfallibleLocalRenderer;
    let mut sink = InMemoryArtifactSink::new();

    // When
    let result = generate(&schema, renderer, &mut sink);

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
    let record = record_schema().resolve().expect("fixture must resolve");
    let schema = PreparedSchema::record("record", &record);
    let renderer = LocalRenderer::default();
    let mut sink = RejectingSink::default();

    // When
    let result = generate(&schema, renderer, &mut sink);

    // Then
    assert_eq!(
        result,
        Err(ExportError::Sink(LocalExportError::SinkRejected))
    );
    assert_eq!(sink.attempted, 1);
}

#[test]
fn renderer_rejection_does_not_invoke_the_sink() {
    // Given
    let record = record_schema().resolve().expect("fixture must resolve");
    let schema = PreparedSchema::record("record", &record);
    let renderer = RejectingRenderer;
    let mut sink = CountingSink::default();

    // When
    let result = generate(&schema, renderer, &mut sink);

    // Then
    assert_eq!(
        result,
        Err(ExportError::Render(LocalExportError::RendererRejected))
    );
    assert_eq!(sink.emitted, 0);
}
