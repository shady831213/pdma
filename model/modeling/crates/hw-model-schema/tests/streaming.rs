//! Caller-owned writers and preparation ordering.
use hw_model_schema::export::{ExportError, OutputMetadata};
use std::io::{self, BufWriter, Write};
/// Explicit output staging for tests.
pub mod output_support;
use hw_model_schema::{
    analysis::{AnalyzedSchema, Bindings},
    ast::SchemaSet,
    export::{generate, record, value},
    record_schema, register_schema,
};
use output_support::MemoryOutput;
record_schema! { pub record Packet { endian: Little, fields: { word: u32 } } }
register_schema! { pub register Register {width:4,endian:Little,access:ReadWrite,reset:Zero(0),writable_mask:0xffff_ffff,fields:{}} }

struct TextBackend;
impl value::Backend for TextBackend {
    type Domain = record::Domain;
    type Error = io::Error;
    type Context<'a> = hw_model_schema::traversal::record::Context<'a>;
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/plain".into(),
        })
    }
}
impl record::Backend for TextBackend {
    fn root(&mut self, _: Self::Context<'_>, writer: &mut dyn Write) -> io::Result<()> {
        writer.write_all(b"first:")?;
        writer.write_all(b"second")
    }
}
fn schema() -> Result<AnalyzedSchema, hw_model_schema::analysis::AnalysisError> {
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    AnalyzedSchema::analyze(set, &Bindings::new())
}

#[derive(Default)]
struct LimitedWriter {
    bytes: Vec<u8>,
    limit: Option<usize>,
    interrupted: bool,
    fail_flush: bool,
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interrupted {
            self.interrupted = false;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let remaining = self
            .limit
            .unwrap_or(usize::MAX)
            .saturating_sub(self.bytes.len());
        if remaining == 0 {
            return Err(io::Error::from_raw_os_error(28));
        }
        let count = bytes.len().min(2).min(remaining);
        self.bytes.extend(bytes.iter().take(count).copied());
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::from_raw_os_error(5))
        } else {
            Ok(())
        }
    }
}

#[test]
fn generation_handles_short_and_interrupted_writes() {
    // Given a writer that interrupts once and accepts only two bytes per write.
    let mut output = LimitedWriter {
        interrupted: true,
        ..LimitedWriter::default()
    };
    // When the actual backend writes both chunks.
    generate(&schema().unwrap(), TextBackend, &mut output).unwrap();
    // Then write_all delivers the complete stream.
    assert_eq!(output.bytes, b"first:second");
}

#[test]
fn write_failure_keeps_prefix_and_original_io_error() {
    // Given a destination with seven bytes of remaining space.
    let mut output = LimitedWriter {
        limit: Some(7),
        ..LimitedWriter::default()
    };
    // When generation runs out of space inside the second chunk.
    let result = generate(&schema().unwrap(), TextBackend, &mut output);
    // Then backend propagation preserves the OS error and committed prefix.
    assert!(matches!(result, Err(ExportError::Render(error)) if error.raw_os_error() == Some(28)));
    assert_eq!(output.bytes, b"first:s");
}

#[test]
fn flush_failure_is_owned_by_the_caller() {
    // Given a writer that rejects flush.
    let mut output = LimitedWriter {
        fail_flush: true,
        ..LimitedWriter::default()
    };
    // When generation completes without flushing.
    generate(&schema().unwrap(), TextBackend, &mut output).unwrap();
    // Then bytes remain available and caller flush preserves the OS error.
    assert_eq!(output.bytes, b"first:second");
    assert_eq!(output.flush().unwrap_err().raw_os_error(), Some(5));
}

#[test]
fn buffered_and_direct_generation_match() {
    // Given direct and buffered caller-selected storage.
    let mut direct = Vec::new();
    let mut buffered = BufWriter::new(Vec::new());
    // When the same schema is rendered through both.
    generate(&schema().unwrap(), TextBackend, &mut direct).unwrap();
    generate(&schema().unwrap(), TextBackend, &mut buffered).unwrap();
    // Then successful roots are fully visible and identical.
    assert_eq!(direct, b"first:second");
    assert!(buffered.get_ref().is_empty());
    buffered.flush().unwrap();
    assert_eq!(buffered.get_ref(), &direct);
}

#[test]
fn metadata_reaches_the_destination_with_root_bytes() {
    // Given the explicit staged test destination.
    let mut output = MemoryOutput::new();
    // When generation identifies and writes its root.
    output.generate(&schema().unwrap(), TextBackend).unwrap();
    // Then the destination receives semantic metadata alongside completed bytes.
    let outputs = output.into_outputs();
    assert_eq!(outputs.len(), 1);
    let captured = outputs.first().unwrap();
    assert_eq!(captured.metadata.logical_name, "Packet");
    assert_eq!(captured.metadata.media_type, "text/plain");
    assert_eq!(captured.bytes, b"first:second");
}

#[test]
fn preflight_rejects_incompatible_units_before_writing() {
    // Given a mixed-domain schema.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    set.add::<Register>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut output = Vec::new();
    // When the record backend validates the entire graph.
    let result = generate(&schema, TextBackend, &mut output);
    // Then domain failure precedes every write.
    assert!(matches!(result, Err(ExportError::Analysis(_))));
    assert!(output.is_empty());
}

record_schema! { pub record SecondPacket { endian: Little, fields: { word: u32 } } }
struct RejectSecondMetadata(usize);
impl value::Backend for RejectSecondMetadata {
    type Domain = record::Domain;
    type Error = io::Error;
    type Context<'a> = hw_model_schema::traversal::record::Context<'a>;
    fn metadata(&mut self, _: &Self::Context<'_>) -> io::Result<OutputMetadata> {
        self.0 += 1;
        if self.0 == 2 {
            Err(io::Error::from(io::ErrorKind::InvalidInput))
        } else {
            Ok(OutputMetadata {
                logical_name: "sample".into(),
                media_type: "text/plain".into(),
            })
        }
    }
}
impl record::Backend for RejectSecondMetadata {
    fn root(&mut self, _: Self::Context<'_>, writer: &mut dyn Write) -> io::Result<()> {
        writer.write_all(b"unexpected")
    }
}
#[test]
fn metadata_failure_precedes_every_write() {
    // Given two roots whose second metadata callback fails.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    set.add::<SecondPacket>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let mut output = Vec::new();
    // When generation preflights output metadata.
    let result = generate(&schema, RejectSecondMetadata(0), &mut output);
    // Then no first-root bytes escape.
    assert!(matches!(result, Err(ExportError::Render(_))));
    assert!(output.is_empty());
}

struct FailRender;
impl value::Backend for FailRender {
    type Domain = record::Domain;
    type Error = io::Error;
    type Context<'a> = hw_model_schema::traversal::record::Context<'a>;
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> io::Result<OutputMetadata> {
        TextBackend.metadata(ctx)
    }
}
impl record::Backend for FailRender {
    fn root(&mut self, _: Self::Context<'_>, writer: &mut dyn Write) -> io::Result<()> {
        writer.write_all(b"partial")?;
        Err(io::Error::from(io::ErrorKind::InvalidData))
    }
}
#[test]
fn render_failure_retains_direct_prefix() {
    // Given a direct destination.
    let mut bytes = Vec::new();
    // When the backend fails after writing a prefix.
    let result = generate(&schema().unwrap(), FailRender, &mut bytes);
    // Then the original error and partial bytes remain.
    assert!(
        matches!(result, Err(ExportError::Render(e)) if e.kind() == io::ErrorKind::InvalidData)
    );
    assert_eq!(bytes, b"partial");
}
#[test]
fn staging_discards_failed_root_and_retains_previous_root() {
    // Given an already published root.
    let mut output = MemoryOutput::new();
    output.generate(&schema().unwrap(), TextBackend).unwrap();
    // When a subsequent root fails during staged rendering.
    let result = output.generate(&schema().unwrap(), FailRender);
    // Then only the previous completed root remains.
    assert!(matches!(result, Err(ExportError::Render(_))));
    assert_eq!(output.outputs().len(), 1);
    assert_eq!(output.outputs().first().unwrap().bytes, b"first:second");
}
#[test]
fn staging_publishes_nothing_when_first_root_fails() {
    // Given empty staged storage.
    let mut output = MemoryOutput::new();
    // When its first render fails.
    let result = output.generate(&schema().unwrap(), FailRender);
    // Then partial bytes are never published.
    assert!(matches!(result, Err(ExportError::Render(_))));
    assert!(output.outputs().is_empty());
}
#[test]
fn selected_mixed_graph_roots_keep_metadata_identity_and_separate_bytes() {
    use hw_model_schema::{export::prepare_roots, traversal::record::roots};
    // Given explicitly selected record roots in a mixed graph.
    let mut set = SchemaSet::new();
    set.add::<Packet>();
    set.add::<SecondPacket>();
    set.add::<Register>();
    let schema = AnalyzedSchema::analyze(set, &Bindings::new()).unwrap();
    let ids: Vec<_> = roots(&schema).map(|v| v.context().id().clone()).collect();
    let mut backend = TextBackend;
    // When the application prepares and routes each unit into its own staging buffer.
    let prepared = prepare_roots(&mut backend, roots(&schema)).unwrap();
    let mut captured = Vec::new();
    for output in &prepared {
        let mut bytes = Vec::new();
        output.emit(&mut backend, &mut bytes).unwrap();
        captured.push((output.metadata().logical_name.clone(), bytes));
    }
    // Then each bound identity and independently staged output survives.
    assert_eq!(
        prepared.iter().map(|p| p.id().clone()).collect::<Vec<_>>(),
        ids
    );
    assert_eq!(
        captured,
        [
            ("Packet".into(), b"first:second".to_vec()),
            ("SecondPacket".into(), b"first:second".to_vec())
        ]
    );
}
