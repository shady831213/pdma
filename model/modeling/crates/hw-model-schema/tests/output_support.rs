//! Explicit per-root staging used only by exporter tests.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Pipeline},
    export::{ExportError, OutputMetadata, prepare, value},
};

#[derive(Debug, PartialEq, Eq)]
/// One successfully published root.
pub struct CapturedOutput {
    /// Identity supplied before rendering.
    pub metadata: OutputMetadata,
    /// Completed rendered bytes.
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
/// Stages each root and retains it only after successful rendering.
pub struct MemoryOutput {
    outputs: Vec<CapturedOutput>,
}
impl MemoryOutput {
    /// Creates an empty staged destination.
    pub const fn new() -> Self {
        Self {
            outputs: Vec::new(),
        }
    }
    /// Borrows completed roots in publication order.
    pub fn outputs(&self) -> &[CapturedOutput] {
        &self.outputs
    }
    /// Transfers completed roots to the caller.
    pub fn into_outputs(self) -> Vec<CapturedOutput> {
        self.outputs
    }
}
impl MemoryOutput {
    /// Prepares all roots, then stages and publishes each successful root.
    pub fn generate<B: value::Backend>(
        &mut self,
        schema: &AnalyzedSchema,
        mut backend: B,
    ) -> Result<(), ExportError<B::Error>> {
        for output in prepare(schema, &mut backend)? {
            let mut bytes = Vec::new();
            output
                .emit(&mut backend, &mut bytes)
                .map_err(ExportError::Render)?;
            self.outputs.push(CapturedOutput {
                metadata: output.metadata().clone(),
                bytes,
            });
        }
        Ok(())
    }
    /// Constructs a backend borrowing previously analyzed data.
    pub fn generate_prepared<'b, D, B: value::Backend>(
        &mut self,
        pipeline: &'b Pipeline<(&AnalyzedSchema, D)>,
        backend: impl FnOnce(&'b D) -> B,
    ) -> Result<(), ExportError<B::Error>> {
        let (schema, data) = pipeline.input();
        self.generate(schema, backend(data))
    }
}
