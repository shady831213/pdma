//! Explicit per-root staging used only by exporter tests.
use hw_model_schema::{
    analysis::{AnalyzedSchema, Pipeline},
    export::{ExportError, prepare},
};

#[derive(Debug, PartialEq, Eq)]
/// One successfully published root.
pub struct CapturedOutput {
    /// Identity supplied before rendering.
    pub entry_name: String,
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
    pub fn generate<B: hw_model_schema::export::value::Backend>(
        &mut self,
        schema: &AnalyzedSchema,
        backend: B,
    ) -> Result<(), ExportError<B::Error>> {
        self.generate_named(schema, backend, str::to_owned)
    }
    /// The caller decides destination names; a backend never chooses files or MIME.
    pub fn generate_named<B: hw_model_schema::export::value::Backend>(
        &mut self,
        schema: &AnalyzedSchema,
        mut backend: B,
        mut name: impl FnMut(&str) -> String,
    ) -> Result<(), ExportError<B::Error>> {
        for output in prepare(schema, &mut backend)? {
            let mut bytes = Vec::new();
            output
                .emit(&mut backend, &mut bytes)
                .map_err(ExportError::Render)?;
            self.outputs.push(CapturedOutput {
                entry_name: name(output.name()),
                bytes,
            });
        }
        Ok(())
    }
    /// Constructs a backend borrowing previously analyzed data.
    pub fn generate_prepared<'b, D, B: hw_model_schema::export::value::Backend>(
        &mut self,
        pipeline: &'b Pipeline<(&AnalyzedSchema, D)>,
        backend: impl FnOnce(&'b D) -> B,
    ) -> Result<(), ExportError<B::Error>> {
        let (schema, data) = pipeline.input();
        self.generate(schema, backend(data))
    }
}
