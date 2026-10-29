//! Veryl layout metadata, without register storage, effects or application policy.
mod geometry;
mod leaves;
use crate::{
    export::{Artifact, ArtifactSink, ExportError, Exporter},
    graph::{Definition, DefinitionBody, DefinitionId, LayoutError, SchemaGraph},
};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// Unsupported or ambiguous target-language layout metadata.
#[derive(Debug)]
pub enum VerylExportError {
    /// Formatting failed while producing an artifact.
    Format(std::fmt::Error),
    /// An invalid or colliding target symbol.
    Name(String),
    /// Invalid parameterized instance geometry or untranslatable expression.
    Layout(LayoutError),
    /// Register geometry is not a supported scalar layout.
    Register(String),
}
impl std::fmt::Display for VerylExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Veryl layout export failed: {self:?}")
    }
}
impl std::error::Error for VerylExportError {}
impl From<LayoutError> for VerylExportError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}
impl From<std::fmt::Error> for VerylExportError {
    fn from(value: std::fmt::Error) -> Self {
        Self::Format(value)
    }
}

/// Emits one deterministic package per definition. Parameterized maps retain
/// their count/argument expressions; application code supplies no address formulas.
#[derive(Debug, Default)]
pub struct VerylLayoutExporter {
    names: HashMap<DefinitionId, String>,
}
impl VerylLayoutExporter {
    /// Uses `<DefinitionName>Layout` package names by default.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides one package name without changing its schema identity.
    pub fn set_name(&mut self, definition: DefinitionId, name: &str) {
        self.names.insert(definition, name.to_owned());
    }
    fn name(&self, definition: &Definition) -> String {
        self.names
            .get(&definition.id())
            .cloned()
            .unwrap_or_else(|| format!("{}Layout", definition.name()))
    }
    fn render(&self, definition: &Definition) -> Result<Artifact, VerylExportError> {
        let name = self.name(definition);
        let mut text = format!(
            "// Generated schema layout; do not edit.\n#[fmt(skip)]\npub package {name} {{\n"
        );
        match definition.body() {
            DefinitionBody::Bits(bits) => {
                write!(
                    text,
                    "    const EXTENT: u64 = {};\n    const ALIGNMENT: u64 = {};\n    const BIT_WIDTH: u32 = {};\n    const DECLARED_MASK: u64 = 64'h{:016x};\n",
                    bits.scalar().width(),
                    bits.scalar().width(),
                    bits.bit_width(),
                    bits.declared_mask()
                )?;
            }
            DefinitionBody::Record(spec)
            | DefinitionBody::Opaque(spec)
            | DefinitionBody::Union { storage: spec, .. } => {
                write!(
                    text,
                    "    const EXTENT: u64 = {};\n    const ALIGNMENT: u64 = {};\n",
                    spec.size(),
                    spec.alignment()
                )?;
            }
            DefinitionBody::Register(register) => {
                write!(
                    text,
                    "    const EXTENT: u64 = {};\n    const ALIGNMENT: u64 = {};\n",
                    register.width(),
                    register.alignment()
                )?;
                leaves::register(&mut text, "VALUE", 0, register)?;
            }
            DefinitionBody::RegisterMap(map) => {
                geometry::render(&mut text, map, self)?;
                if map.parameters().is_empty() {
                    let layout = map.resolve(&[])?;
                    writeln!(text, "    const EXTENT: u64 = {};", layout.extent)?;
                    leaves::map(&mut text, map, &[], 0, "", &mut HashSet::new())?;
                }
            }
        }
        text.push_str("}\n");
        Ok(Artifact {
            logical_name: format!("{name}.veryl"),
            media_type: "text/x-veryl; charset=utf-8".into(),
            contents: text.into_bytes(),
        })
    }
}
impl Exporter<SchemaGraph> for VerylLayoutExporter {
    type Error = VerylExportError;
    fn export<Sink: ArtifactSink>(
        &self,
        graph: &SchemaGraph,
        sink: &mut Sink,
    ) -> Result<(), ExportError<Self::Error, Sink::Error>> {
        let mut names = HashSet::new();
        let mut artifacts = Vec::new();
        for definition in graph.dependency_order() {
            let name = self.name(definition);
            if !identifier(&name) || !names.insert(name.clone()) {
                return Err(ExportError::Render(VerylExportError::Name(name)));
            }
            artifacts.push(self.render(definition).map_err(ExportError::Render)?);
        }
        for artifact in artifacts {
            sink.emit(artifact).map_err(ExportError::Sink)?;
        }
        Ok(())
    }
}
fn identifier(value: &str) -> bool {
    value.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
