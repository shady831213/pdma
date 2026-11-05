//! Downstream-style checks for the format-neutral export extension boundary.
/// Caller-owned capture for generation tests.
pub mod output_support;
use hw_model_schema::{
    RecordSpec,
    analysis::AnalyzedSchema,
    export::{ExportError, OutputMetadata, generate, record::Backend, value},
    traversal::record::Context as RecordContext,
};
use output_support::{CapturedOutput, MemoryOutput};
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalExportError {
    RendererRejected,
    Write(std::io::ErrorKind),
}
#[derive(Default)]
struct RejectingWriter {
    attempted: usize,
}
impl Write for RejectingWriter {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        self.attempted += 1;
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct LocalRenderer;
impl Backend for LocalRenderer {}
impl value::Backend for LocalRenderer {
    type Domain = hw_model_schema::export::record::Domain;
    type Error = LocalExportError;
    type Context<'a> = RecordContext<'a>;
    fn opaque(
        &mut self,
        record: &RecordSpec,
        _: RecordContext<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let size = u8::try_from(record.size()).map_err(|_| LocalExportError::RendererRejected)?;
        writer
            .write_all(&[size, 255, 7])
            .map_err(|error| LocalExportError::Write(error.kind()))
    }
    fn metadata(&mut self, ctx: &RecordContext<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: format!("{}.bin", ctx.unit().name),
            media_type: "application/x-local-schema".into(),
        })
    }
}
struct RejectingRenderer;
impl Backend for RejectingRenderer {
    fn root(&mut self, _: RecordContext<'_>, _: &mut dyn Write) -> Result<(), Self::Error> {
        Err(LocalExportError::RendererRejected)
    }
}
impl value::Backend for RejectingRenderer {
    type Domain = hw_model_schema::export::record::Domain;
    type Error = LocalExportError;
    type Context<'a> = RecordContext<'a>;
    fn metadata(&mut self, ctx: &RecordContext<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/plain".into(),
        })
    }
}
const fn record_schema() -> RecordSpec {
    RecordSpec::new(0, 1, &[])
}
#[test]
fn caller_capture_preserves_owned_odd_contents_after_renderer_locals_drop() {
    // Given: a local renderer with binary output and caller-selected capture storage.
    let schema = AnalyzedSchema::opaque("record", record_schema()).expect("fixture must resolve");
    let mut output = MemoryOutput::new();
    // When: the renderer writes through the caller's writer.
    let result = output.generate(&schema, LocalRenderer);
    // Then: the caller retains exact metadata and non-text bytes.
    assert_eq!(result, Ok(()));
    assert_eq!(
        output.outputs(),
        &[CapturedOutput {
            metadata: OutputMetadata {
                logical_name: "record.bin".into(),
                media_type: "application/x-local-schema".into(),
            },
            bytes: vec![0, 255, 7],
        }]
    );
}
#[test]
fn writer_rejection_reaches_the_caller_as_the_exact_typed_error() {
    // Given: the caller supplies a writer that rejects byte writes.
    let schema = AnalyzedSchema::opaque("record", record_schema()).expect("fixture must resolve");
    let mut output = RejectingWriter::default();
    // When: generation writes the root output.
    let result = generate(&schema, LocalRenderer, &mut output);
    // Then: the backend preserves the IO error kind and only one write was attempted.
    assert_eq!(
        result,
        Err(ExportError::Render(LocalExportError::Write(
            std::io::ErrorKind::PermissionDenied
        )))
    );
    assert_eq!(output.attempted, 1);
}
#[test]
fn renderer_rejection_does_not_publish_staged_output() {
    // Given: rendering rejects the root with application-owned staging.
    let schema = AnalyzedSchema::opaque("record", record_schema()).expect("fixture must resolve");
    let mut output = MemoryOutput::new();
    // When: generation invokes the root callback.
    let result = output.generate(&schema, RejectingRenderer);
    // Then: the typed rendering failure prevents completion of that output.
    assert_eq!(
        result,
        Err(ExportError::Render(LocalExportError::RendererRejected))
    );
    assert!(output.outputs().is_empty());
}

#[test]
fn downstream_local_renderer_and_output_preserve_root_emission_order() {
    use hw_model_schema::{
        analysis::Bindings,
        ast::{Definition, SchemaSet},
    };
    struct First;
    struct Second;
    // Given: two selected opaque definitions added in reverse name order.
    let mut set = SchemaSet::new();
    set.add_definition(Definition::opaque::<Second>(RecordSpec::named(
        "second",
        8,
        1,
        &[],
        &[],
    )));
    set.add_definition(Definition::opaque::<First>(RecordSpec::named(
        "first",
        4,
        1,
        &[],
        &[],
    )));
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut output = MemoryOutput::new();
    // When: generation visits each root through the downstream implementation.
    output.generate(&schema, LocalRenderer).unwrap();
    // Then: the caller observes stable root ordering and independent binary contents.
    let actual: Vec<_> = output
        .outputs()
        .iter()
        .map(|value| (value.metadata.logical_name.as_str(), value.bytes.as_slice()))
        .collect();
    assert_eq!(
        actual,
        [
            ("first.bin", &[4, 255, 7][..]),
            ("second.bin", &[8, 255, 7][..])
        ]
    );
}
