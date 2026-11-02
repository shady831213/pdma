use super::{DownPair, TelemetryMap};
use core::convert::Infallible;
use his::analysis::{AnalyzedSchema, Bindings, Layout, RegisterMapView, UnitId};
use his::export::value::Backend as _;
use his::{
    ast::{DefinitionId, RecordDefinition, SchemaSet},
    export::{self, Artifact, ArtifactSink, InMemoryArtifactSink, value},
};
use std::fmt::Write as _;

/// Application-owned artifact collector with no filesystem policy.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TelemetrySink {
    artifacts: Vec<Artifact>,
}
impl TelemetrySink {
    /// Returns artifacts in their downstream-defined emission order.
    #[must_use]
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }
}
impl ArtifactSink for TelemetrySink {
    type Error = Infallible;
    fn emit(&mut self, artifact: Artifact) -> Result<(), Self::Error> {
        self.artifacts.push(artifact);
        Ok(())
    }
}
/// Application-owned record report renderer.
#[derive(Debug, Default)]
pub struct TelemetryRecordRenderer {
    content: String,
}
/// Application-owned register report renderer.
#[derive(Debug, Default)]
pub struct TelemetryRegisterRenderer {
    content: String,
}

/// Assembles independently generated record and register reports in application order.
pub fn export_resolved(sink: &mut TelemetrySink) -> Option<()> {
    let mut records = SchemaSet::new();
    records.add_record::<DownPair>();
    let mut registers = SchemaSet::new();
    registers.add::<TelemetryMap>();
    let records = AnalyzedSchema::from_graph(&records.resolve().ok()?, &Bindings::new()).ok()?;
    let registers =
        AnalyzedSchema::from_graph(&registers.resolve().ok()?, &Bindings::new()).ok()?;
    let mut fragments = InMemoryArtifactSink::new();
    export::generate(&records, TelemetryRecordRenderer::default(), &mut fragments).ok()?;
    export::generate(
        &registers,
        TelemetryRegisterRenderer::default(),
        &mut fragments,
    )
    .ok()?;
    sink.emit(Artifact {
        logical_name: "acme-telemetry-layout.txt".into(),
        media_type: "text/vnd.acme.telemetry-layout".into(),
        contents: fragments
            .into_artifacts()
            .into_iter()
            .flat_map(|artifact| artifact.contents)
            .collect(),
    })
    .ok()
}
impl export::record::Backend for TelemetryRecordRenderer {
    fn root(&mut self, ctx: export::record::EmitContext<'_>) -> Result<(), Self::Error> {
        if ctx.unit().id == UnitId::Definition(DefinitionId::of::<DownPair>()) {
            self.emit(ctx.current())?;
        }
        Ok(())
    }
    fn record(
        &mut self,
        _: &RecordDefinition,
        ctx: export::record::EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        let Layout::Record(record) = &ctx.unit().body else {
            return Err(std::io::Error::other("record layout"));
        };
        write!(self.content, "record_size={};record=", record.size())
            .map_err(std::io::Error::other)?;
        for (index, member) in record.members().iter().enumerate() {
            if index > 0 {
                self.content.push(',');
            }
            write!(
                self.content,
                "{}@{}+{}",
                member.path(),
                member.absolute_offset(),
                member.range().len()
            )
            .map_err(std::io::Error::other)?;
        }
        self.content.push(';');
        Ok(())
    }
}
impl value::Backend for TelemetryRecordRenderer {
    type Domain = export::record::Domain;
    type Error = std::io::Error;
    type Context<'a> = export::record::EmitContext<'a>;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "records.txt".into(),
            media_type: "text/plain".into(),
            contents: self.content.into_bytes(),
        }])
    }
}
impl export::register::Backend for TelemetryRegisterRenderer {
    fn root(&mut self, ctx: export::register::EmitContext<'_>) -> Result<(), Self::Error> {
        if ctx.unit().id == UnitId::Definition(DefinitionId::of::<TelemetryMap>()) {
            self.emit(ctx.current())?;
        }
        Ok(())
    }
    fn register_map(
        &mut self,
        map: &RegisterMapView,
        _: export::register::EmitContext<'_>,
    ) -> Result<(), Self::Error> {
        write!(self.content, "map_size={};registers=", map.extent)
            .map_err(std::io::Error::other)?;
        for (index, view) in map.registers.iter().enumerate() {
            if index > 0 {
                self.content.push(',');
            }
            let path = view
                .path
                .strip_prefix("TelemetryMap.")
                .map_or(view.path.as_str(), |path| path);
            write!(self.content, "{path}@{}", view.offset).map_err(std::io::Error::other)?;
        }
        Ok(())
    }
}
impl value::Backend for TelemetryRegisterRenderer {
    type Domain = export::register::Domain;
    type Error = std::io::Error;
    type Context<'a> = export::register::EmitContext<'a>;
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: "registers.txt".into(),
            media_type: "text/plain".into(),
            contents: self.content.into_bytes(),
        }])
    }
}
