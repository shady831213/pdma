use super::{DownPair, TelemetryMap};
use core::convert::Infallible;
use his::{ResolvedMember, ResolvedRecord};
use his::{
    export::{
        Artifact, ArtifactSink, Bindings, DefinitionHeader, ExportId, PreparedSchema,
        RegisterLocation, RegisterMapView, RegisterNode, Renderer, generate,
    },
    graph::{DefinitionId, SchemaSet},
};
use std::fmt::Write as _;

/// Application-owned artifact collector with no filesystem policy.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TelemetrySink {
    artifacts: Vec<Artifact>,
}

impl TelemetrySink {
    /// Returns the artifacts in their downstream-defined emission order.
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

/// Application-owned renderer for the selected telemetry definitions.
#[derive(Debug, Default)]
pub struct TelemetryRenderer {
    content: String,
    target: Target,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum Target {
    #[default]
    Other,
    Record,
    Map,
}

/// Exports fresh resolved graphs through an application-owned sink.
pub fn export_resolved(sink: &mut TelemetrySink) -> Option<()> {
    let mut selected = SchemaSet::new();
    selected.add_record::<DownPair>();
    selected.add::<TelemetryMap>();
    let prepared = PreparedSchema::from_graph(&selected.resolve().ok()?, &Bindings::new()).ok()?;
    generate(&prepared, TelemetryRenderer::default(), sink).ok()
}

impl Renderer for TelemetryRenderer {
    type Error = std::io::Error;

    fn begin_definition(&mut self, definition: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        self.target = if *definition.id == ExportId::Definition(DefinitionId::of::<DownPair>()) {
            Target::Record
        } else if *definition.id == ExportId::Definition(DefinitionId::of::<TelemetryMap>()) {
            Target::Map
        } else {
            Target::Other
        };
        Ok(())
    }

    fn begin_record(&mut self, record: &ResolvedRecord) -> Result<(), Self::Error> {
        if self.target == Target::Record {
            write!(self.content, "record_size={};record=", record.size())
                .map_err(std::io::Error::other)?;
        }
        Ok(())
    }

    fn record_member(&mut self, index: usize, member: &ResolvedMember) -> Result<(), Self::Error> {
        if self.target == Target::Record {
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
        Ok(())
    }

    fn end_record(&mut self, _: &ResolvedRecord) -> Result<(), Self::Error> {
        if self.target == Target::Record {
            self.content.push(';');
        }
        Ok(())
    }

    fn begin_map_instance(&mut self, map: &RegisterMapView) -> Result<(), Self::Error> {
        if self.target == Target::Map {
            write!(self.content, "map_size={};registers=", map.extent)
                .map_err(std::io::Error::other)?;
        }
        Ok(())
    }

    fn begin_register(&mut self, node: RegisterNode<'_>) -> Result<(), Self::Error> {
        match node.location {
            RegisterLocation::Standalone(index) if self.target == Target::Map => {
                let view = node
                    .view
                    .ok_or_else(|| std::io::Error::other("map register view"))?;
                if index > 0 {
                    self.content.push(',');
                }
                let path = view
                    .path
                    .strip_prefix("TelemetryMap.")
                    .unwrap_or(&view.path);
                write!(self.content, "{path}@{}", view.offset).map_err(std::io::Error::other)?;
            }
            RegisterLocation::Root
            | RegisterLocation::Standalone(_)
            | RegisterLocation::Template { .. }
            | RegisterLocation::Instance { .. } => {}
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
