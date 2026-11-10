use super::{DownPair, TelemetryMap};
use his::analysis::{AnalyzedSchema, Bindings};
use his::{ast::SchemaSet, export};
mod record;
mod register;
pub use record::TelemetryRecordRenderer;
pub use register::TelemetryRegisterRenderer;

/// Completed application report, independent of framework storage policy.
#[derive(Debug, PartialEq, Eq)]
pub struct TelemetryReport {
    /// Application-selected report name.
    pub logical_name: String,
    /// Application-selected report format.
    pub media_type: String,
    /// Report bytes explicitly retained by the application.
    pub contents: Vec<u8>,
}
/// Application-owned report collector with per-root staging.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TelemetrySink {
    artifacts: Vec<TelemetryReport>,
}
impl TelemetrySink {
    /// Returns artifacts in their downstream-defined emission order.
    #[must_use]
    pub fn artifacts(&self) -> &[TelemetryReport] {
        &self.artifacts
    }
}
/// Assembles independently generated record and register reports in application order.
pub fn export_resolved(sink: &mut TelemetrySink) -> Result<(), Box<dyn std::error::Error>> {
    let mut records = SchemaSet::new();
    records.add_record::<DownPair>();
    let mut registers = SchemaSet::new();
    registers.add::<TelemetryMap>();
    let records = AnalyzedSchema::from_graph(&records.resolve()?, &Bindings::new())?;
    let registers = AnalyzedSchema::from_graph(&registers.resolve()?, &Bindings::new())?;
    let mut fragments = Vec::new();
    export::generate(
        &records,
        TelemetryRecordRenderer::new(&records)?,
        &mut fragments,
    )?;
    export::generate(
        &registers,
        TelemetryRegisterRenderer::new(&registers)?,
        &mut fragments,
    )?;
    sink.artifacts.push(TelemetryReport {
        logical_name: "acme-telemetry-layout.txt".into(),
        media_type: "text/vnd.acme.telemetry-layout".into(),
        contents: fragments,
    });
    Ok(())
}
