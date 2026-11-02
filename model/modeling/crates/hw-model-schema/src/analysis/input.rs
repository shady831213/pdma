use crate::{
    RecordGraphError,
    ast::{DefinitionId, DescribeSchema, LayoutError},
};
use std::collections::HashMap;
/// Generator-supplied root instance parameters; child arguments follow schema references.
#[derive(Debug, Clone, Default)]
pub struct Bindings {
    pub(super) values: HashMap<DefinitionId, Vec<usize>>,
}
impl Bindings {
    /// Creates a symbolic generation request without bound roots.
    pub fn new() -> Self {
        Self::default()
    }
    /// Binds one selected, unreferenced root to concrete instance arguments.
    pub fn bind<T: DescribeSchema + 'static>(&mut self, arguments: &[usize]) -> &mut Self {
        self.values
            .insert(DefinitionId::of::<T>(), arguments.to_vec());
        self
    }
}

/// Schema preparation failed before any renderer or sink was called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    /// A selected definition belongs to another source domain.
    Domain {
        /// Selected definition name.
        name: String,
        /// Requested export domain.
        domain: &'static str,
    },
    /// Invalid dependencies in a manually supplied source declaration.
    Dependencies(crate::ast::GraphError),
    /// A logical source field exceeds or overlaps its record's declared storage.
    SourceField {
        /// Enclosing record name.
        record: &'static str,
        /// Invalid field name.
        field: &'static str,
    },
    /// An analyzed unit has neither a declaration nor a concrete projection.
    MissingDeclaration(String),
    /// Invalid record geometry.
    Record {
        /// Defining Rust path retained for diagnostics.
        path: &'static str,
        /// Original checked record-layout failure.
        source: RecordGraphError,
    },
    /// Invalid whole-register shape.
    Register {
        /// Source register name.
        path: &'static str,
        /// Shared definition validation failure.
        source: crate::RegisterDefinitionError<usize>,
    },
    /// Invalid symbolic or bound register-map geometry.
    Layout(LayoutError),
    /// A binding names a missing, non-map or non-root definition.
    Binding(DefinitionId),
    /// A generic referenced definition cannot also be specialized under the same identity.
    ReferencedBinding(DefinitionId),
}
impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Domain { name, domain } => write!(f, "{name} is not a {domain} schema"),
            Self::Dependencies(error) => error.fmt(f),
            Self::SourceField { record, field } => {
                write!(f, "invalid source field {record}.{field}")
            }
            Self::MissingDeclaration(name) => write!(f, "missing declaration for {name}"),
            Self::Register { path, source } => write!(f, "{path}: {source}"),
            Self::Record { path, source } => write!(f, "{path}: {source}"),
            Self::Layout(error) => error.fmt(f),
            Self::Binding(id) => write!(f, "invalid root binding {}", id.path()),
            Self::ReferencedBinding(id) => write!(
                f,
                "bind {} in a separate root generation request",
                id.path()
            ),
        }
    }
}
impl std::error::Error for AnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dependencies(source) => Some(source),
            Self::Register { source, .. } => Some(source),
            Self::Record { source, .. } => Some(source),
            Self::Layout(source) => Some(source),
            Self::Domain { .. }
            | Self::Binding(_)
            | Self::ReferencedBinding(_)
            | Self::MissingDeclaration(_)
            | Self::SourceField { .. } => None,
        }
    }
}
impl From<LayoutError> for AnalysisError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}
