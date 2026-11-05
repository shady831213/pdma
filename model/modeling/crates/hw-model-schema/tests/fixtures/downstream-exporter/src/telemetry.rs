use super::{DownPair, TelemetryMap};
use his::analysis::{AnalyzedSchema, Bindings, Layout, RegisterMapView};
use his::export::value::Backend as _;
use his::{
    ast::{DefinitionId, RecordDefinition, SchemaSet},
    export::{self, OutputMetadata, value},
};
use std::io::Write;

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
/// Application-owned record report renderer.
#[derive(Debug, Default)]
pub struct TelemetryRecordRenderer;
/// Application-owned register report renderer.
#[derive(Debug, Default)]
pub struct TelemetryRegisterRenderer;

/// Assembles independently generated record and register reports in application order.
pub fn export_resolved(sink: &mut TelemetrySink) -> Result<(), Box<dyn std::error::Error>> {
    let mut records = SchemaSet::new();
    records.add_record::<DownPair>();
    let mut registers = SchemaSet::new();
    registers.add::<TelemetryMap>();
    let records = AnalyzedSchema::from_graph(&records.resolve()?, &Bindings::new())?;
    let registers = AnalyzedSchema::from_graph(&registers.resolve()?, &Bindings::new())?;
    let mut fragments = Vec::new();
    export::generate(&records, TelemetryRecordRenderer, &mut fragments)?;
    export::generate(&registers, TelemetryRegisterRenderer, &mut fragments)?;
    sink.artifacts.push(TelemetryReport {
        logical_name: "acme-telemetry-layout.txt".into(),
        media_type: "text/vnd.acme.telemetry-layout".into(),
        contents: fragments,
    });
    Ok(())
}
impl export::record::Backend for TelemetryRecordRenderer {
    fn root(
        &mut self,
        ctx: his::traversal::record::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx
            .schema()
            .definition(DefinitionId::of::<DownPair>())
            .is_some_and(|unit| unit.id == ctx.unit().id)
        {
            self.emit(ctx.current(), writer)?;
        }
        Ok(())
    }
    fn record(
        &mut self,
        _: &RecordDefinition,
        ctx: his::traversal::record::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let Layout::Record(record) = &ctx.unit().body else {
            return Err(std::io::Error::other("record layout"));
        };
        write!(writer, "record_size={};record=", record.size())?;
        for (index, member) in record.members().iter().enumerate() {
            if index > 0 {
                writer.write_all(b",")?;
            }
            write!(
                writer,
                "{}@{}+{}",
                member.path(),
                member.absolute_offset(),
                member.range().len()
            )?;
        }
        writer.write_all(b";")?;
        Ok(())
    }
}
impl value::Backend for TelemetryRecordRenderer {
    type Domain = export::record::Domain;
    type Error = std::io::Error;
    type Context<'a> = his::traversal::record::Context<'a>;
    fn metadata(&mut self, _: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: "records.txt".into(),
            media_type: "text/plain".into(),
        })
    }
}
impl export::register::Backend for TelemetryRegisterRenderer {
    fn root(
        &mut self,
        ctx: his::traversal::register::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx
            .schema()
            .definition(DefinitionId::of::<TelemetryMap>())
            .is_some_and(|unit| unit.id == ctx.unit().id)
        {
            self.emit(ctx.current(), writer)?;
        }
        Ok(())
    }
    fn register_map(
        &mut self,
        map: &RegisterMapView,
        _: his::traversal::register::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        write!(writer, "map_size={};registers=", map.extent)?;
        for (index, view) in map.registers.iter().enumerate() {
            if index > 0 {
                writer.write_all(b",")?;
            }
            let path = view
                .path
                .strip_prefix("TelemetryMap.")
                .map_or(view.path.as_str(), |path| path);
            write!(writer, "{path}@{}", view.offset)?;
        }
        Ok(())
    }
}
impl value::Backend for TelemetryRegisterRenderer {
    type Domain = export::register::Domain;
    type Error = std::io::Error;
    type Context<'a> = his::traversal::register::Context<'a>;
    fn metadata(&mut self, _: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        Ok(OutputMetadata {
            logical_name: "registers.txt".into(),
            media_type: "text/plain".into(),
        })
    }
}
