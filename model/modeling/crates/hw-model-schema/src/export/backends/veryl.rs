//! Veryl layout metadata, without register storage, effects or application policy.
mod backend;
mod geometry;
mod package;
use crate::ast::analysis::UnitId;
use crate::export::{Artifact, SymbolScope};
use std::collections::HashMap;

/// Unsupported or ambiguous target-language layout metadata.
#[derive(Debug)]
pub enum VerylExportError {
    /// Formatting failed while producing an artifact.
    Format(std::fmt::Error),
    /// Checked layout expression could not be formatted.
    Layout(crate::ast::LayoutError),
    /// An invalid or colliding target symbol.
    Name(String),
    /// Register geometry is not a supported scalar layout.
    Register(String),
}
impl std::fmt::Display for VerylExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format(error) => error.fmt(f),
            Self::Layout(error) => error.fmt(f),
            Self::Name(name) => write!(f, "invalid or conflicting Veryl symbol {name}"),
            Self::Register(name) => write!(f, "unsupported register geometry for {name}"),
        }
    }
}
impl std::error::Error for VerylExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Format(source) => Some(source),
            Self::Layout(source) => Some(source),
            Self::Name(_) | Self::Register(_) => None,
        }
    }
}
impl From<std::fmt::Error> for VerylExportError {
    fn from(value: std::fmt::Error) -> Self {
        Self::Format(value)
    }
}

/// Renders prepared definitions into deterministic Veryl layout packages.
#[derive(Debug, Default)]
pub struct VerylBackend {
    names: HashMap<UnitId, String>,
    symbols: SymbolScope<UnitId>,
    artifacts: Vec<Artifact>,
    package: package::Package,
}
impl VerylBackend {
    /// Uses `<DefinitionName>Layout` package names by default.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides one package name without changing its schema identity.
    #[must_use]
    pub fn with_name(mut self, definition: impl Into<UnitId>, name: &str) -> Self {
        self.names.insert(definition.into(), name.to_owned());
        self
    }
    fn name(&self, id: &UnitId, name: &str) -> String {
        self.names
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("{name}Layout"))
    }
}
fn identifier(value: &str) -> bool {
    value.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
