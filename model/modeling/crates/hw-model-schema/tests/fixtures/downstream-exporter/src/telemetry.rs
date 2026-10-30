use super::{DownPair, TelemetryMap};
use core::convert::Infallible;
use his::{
    ast::{
        DefinitionId, MapView, Node, RecordDefinition, SchemaSet,
        analysis::{Analysis, Bindings, Layout, UnitId},
    },
    export::{Artifact, ArtifactSink, Backend, Scope, emit, generate},
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
/// Application-owned backend for selected telemetry definitions.
#[derive(Debug, Default)]
pub struct TelemetryRenderer {
    content: String,
}

/// Exports fresh resolved graphs through an application-owned sink.
pub fn export_resolved(sink: &mut TelemetrySink) -> Option<()> {
    let mut selected = SchemaSet::new();
    selected.add_record::<DownPair>();
    selected.add::<TelemetryMap>();
    let prepared = Analysis::from_graph(&selected.resolve().ok()?, &Bindings::new()).ok()?;
    generate(&prepared, TelemetryRenderer::default(), sink).ok()
}

impl Backend for TelemetryRenderer {
    type Error = std::io::Error;
    fn definition(&mut self, node: Node<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        if scope.unit.id == UnitId::Definition(DefinitionId::of::<DownPair>())
            || scope.unit.id == UnitId::Definition(DefinitionId::of::<TelemetryMap>())
        {
            emit(node, scope, self)?;
        }
        Ok(())
    }
    fn record(&mut self, _: &RecordDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        let Layout::Record(record) = &scope.unit.body else {
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
    fn register_map(&mut self, _: MapView<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        let map = scope
            .map_layout()
            .and_then(|layout| layout.instance.as_ref())
            .ok_or_else(|| std::io::Error::other("map instance"))?;
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
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(vec![Artifact {
            logical_name: String::from("acme-telemetry-layout.txt"),
            media_type: String::from("text/vnd.acme.telemetry-layout"),
            contents: self.content.into_bytes(),
        }])
    }
}
