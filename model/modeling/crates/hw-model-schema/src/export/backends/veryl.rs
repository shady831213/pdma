//! Veryl layout metadata, without register storage, effects or application policy.
mod package;
mod record;
mod register;
mod values;
use crate::analysis::UnitId;
use crate::export::{Artifact, SymbolScope};
use std::collections::HashMap;

/// Unsupported or ambiguous target-language layout metadata.
#[derive(Debug)]
pub enum VerylExportError {
    /// An expression was not completed by semantic analysis.
    Layout(crate::ast::LayoutError),
    /// Formatting failed while producing an artifact.
    Format(std::fmt::Error),
    /// An invalid or colliding target symbol.
    Name(String),
}
impl std::fmt::Display for VerylExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layout(error) => error.fmt(f),
            Self::Format(error) => error.fmt(f),
            Self::Name(name) => write!(f, "invalid or conflicting Veryl symbol {name}"),
        }
    }
}
impl std::error::Error for VerylExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(source) => Some(source),
            Self::Format(source) => Some(source),
            Self::Name(_) => None,
        }
    }
}
impl From<crate::ast::LayoutError> for VerylExportError {
    fn from(value: crate::ast::LayoutError) -> Self {
        Self::Layout(value)
    }
}
impl From<std::fmt::Error> for VerylExportError {
    fn from(value: std::fmt::Error) -> Self {
        Self::Format(value)
    }
}

/// Target package storage shared by the independent domain renderers.
#[derive(Debug, Default)]
struct Output {
    names: HashMap<UnitId, String>,
    symbols: SymbolScope<UnitId>,
    artifacts: Vec<Artifact>,
    package: package::Package,
}
impl Output {
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

/// Emits record and reusable value layout packages.
#[derive(Debug, Default)]
pub struct VerylRecordBackend {
    output: Output,
}
impl VerylRecordBackend {
    /// Uses the schema name followed by `Layout`.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides one package name.
    #[must_use]
    pub fn with_name(mut self, definition: impl Into<UnitId>, name: &str) -> Self {
        self.output.names.insert(definition.into(), name.into());
        self
    }
}
/// Emits register and register-map layout packages.
#[derive(Debug, Default)]
pub struct VerylRegisterBackend {
    output: Output,
}
impl VerylRegisterBackend {
    /// Uses the schema name followed by `Layout`.
    pub fn new() -> Self {
        Self::default()
    }
    /// Overrides one package name.
    #[must_use]
    pub fn with_name(mut self, definition: impl Into<UnitId>, name: &str) -> Self {
        self.output.names.insert(definition.into(), name.into());
        self
    }
}
impl Output {
    fn begin(&mut self, unit: &crate::analysis::Unit) -> Result<String, VerylExportError> {
        let name = self.name(&unit.id, &unit.name);
        if !identifier(&name) || self.symbols.insert(&name, unit.id.clone()).is_err() {
            return Err(VerylExportError::Name(name));
        }
        self.package = package::Package::default();
        self.package.text = format!(
            "// Generated schema layout; do not edit.\n#[fmt(skip)]\npub package {name} {{\n"
        );
        Ok(name)
    }
    fn end(&mut self, name: &str) {
        self.package.text.push_str("}\n");
        self.artifacts.push(Artifact {
            logical_name: format!("{name}.veryl"),
            media_type: "text/x-veryl; charset=utf-8".into(),
            contents: std::mem::take(&mut self.package.text).into_bytes(),
        });
    }
}
