//! Veryl layout metadata, without register storage, effects or application policy.
mod package;
mod record;
mod register;
mod values;
use crate::export::{OutputMetadata, SymbolScope};
use crate::traversal::NodeId;
use std::collections::HashMap;

/// Unsupported or ambiguous target-language layout metadata.
#[derive(Debug)]
pub enum VerylExportError {
    /// An expression was not completed by semantic analysis.
    Layout(crate::ast::LayoutError),
    /// Writing the output failed.
    Io(std::io::Error),
    /// An invalid or colliding target symbol.
    Name(String),
}
impl std::fmt::Display for VerylExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layout(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::Name(name) => write!(f, "invalid or conflicting Veryl symbol {name}"),
        }
    }
}
impl std::error::Error for VerylExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::Name(_) => None,
        }
    }
}
impl From<crate::ast::LayoutError> for VerylExportError {
    fn from(value: crate::ast::LayoutError) -> Self {
        Self::Layout(value)
    }
}
impl From<std::io::Error> for VerylExportError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Naming state shared by the independent domain renderers.
#[derive(Debug, Default)]
struct Output {
    names: HashMap<NodeId, String>,
    symbols: SymbolScope<NodeId>,
    package: package::Package,
}
impl Output {
    fn name(&self, id: &NodeId, name: &str) -> String {
        self.names
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("{name}Layout"))
    }
}
fn identifier(value: &str) -> bool {
    value.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !KEYWORDS.contains(&value)
}

// Keyword tokens in Veryl 0.22 parser/veryl.par, revision bacc44532af83058bdaab97dbe66ca1b89976be2.
const KEYWORDS: &[&str] = &[
    "alias",
    "always_comb",
    "always_ff",
    "assign",
    "as",
    "bind",
    "bit",
    "block",
    "bbool",
    "lbool",
    "case",
    "clock",
    "clock_posedge",
    "clock_negedge",
    "connect",
    "const",
    "converse",
    "default",
    "else",
    "embed",
    "enum",
    "f32",
    "f64",
    "false",
    "final",
    "for",
    "function",
    "gen",
    "i8",
    "i16",
    "i32",
    "i64",
    "if_reset",
    "if",
    "import",
    "include",
    "initial",
    "inout",
    "input",
    "inside",
    "inst",
    "interface",
    "in",
    "let",
    "logic",
    "lsb",
    "mixin",
    "modport",
    "module",
    "msb",
    "output",
    "outside",
    "package",
    "param",
    "proto",
    "pub",
    "repeat",
    "reset",
    "reset_async_high",
    "reset_async_low",
    "reset_sync_high",
    "reset_sync_low",
    "reset_active",
    "reset_inactive",
    "return",
    "rev",
    "break",
    "same",
    "signed",
    "step",
    "string",
    "struct",
    "switch",
    "tri",
    "true",
    "type",
    "p8",
    "p16",
    "p32",
    "p64",
    "u8",
    "u16",
    "u32",
    "u64",
    "union",
    "unsafe",
    "var",
];

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
    pub fn with_name(mut self, definition: NodeId, name: &str) -> Self {
        self.output.names.insert(definition, name.into());
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
    pub fn with_name(mut self, definition: NodeId, name: &str) -> Self {
        self.output.names.insert(definition, name.into());
        self
    }
}
impl Output {
    fn metadata(
        &mut self,
        unit: &crate::analysis::Unit,
    ) -> Result<OutputMetadata, VerylExportError> {
        let name = self.name(&unit.id, &unit.name);
        if !identifier(&name) || self.symbols.insert(&name, unit.id.clone()).is_err() {
            return Err(VerylExportError::Name(name));
        }
        Ok(OutputMetadata {
            logical_name: format!("{name}.veryl"),
            media_type: "text/x-veryl; charset=utf-8".into(),
        })
    }
    fn begin(
        &mut self,
        unit: &crate::analysis::Unit,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), VerylExportError> {
        self.package = package::Package::default();
        writeln!(
            writer,
            "// Generated schema layout; do not edit.\n#[fmt(skip)]\npub package {} {{",
            self.name(&unit.id, &unit.name)
        )?;
        Ok(())
    }
}
