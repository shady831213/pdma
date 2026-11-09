//! Optional source rewrites, followed by ordinary resolution and analysis.
//!
//! Selection is local to a definition's source occurrence, not to each expanded
//! use. Bits, records, unions and opaque layouts are reusable value definitions;
//! scalar and enumeration metadata remain leaves. Map children retain their own
//! argument scope. Lifted unions bind external selectors on their reference edge. These passes never transform permission-only projections.
mod engine;
mod record;
mod register;
use crate::{
    analysis::Pass,
    ast::{DefinitionId, GraphError, SchemaGraph, SchemaSet},
};

/// One definition-local field, case, placement or register-source occurrence.
#[derive(Debug, Clone)]
pub struct Occurrence {
    pub(crate) owner: DefinitionId,
    pub(crate) owner_name: String,
    pub(crate) path: Vec<&'static str>,
    pub(crate) identity: Vec<String>,
}
impl Occurrence {
    /// Source owner identity, including any prior derivation.
    pub const fn owner(&self) -> &DefinitionId {
        &self.owner
    }
    /// Member names from the owning definition, without following references.
    pub fn path(&self) -> &[&'static str] {
        &self.path
    }
    pub(crate) fn child(&self, role: &str, index: usize, name: &'static str) -> Self {
        let mut child = self.clone();
        child.path.push(name);
        child.identity.push(format!("{role}:{index}:{name}"));
        child
    }
}
/// A selected source rewrite could not preserve the source contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformError {
    /// Invalid source dependencies.
    Graph(GraphError),
    /// A selected edge targets a definition outside its containment domain.
    Unsupported {
        /// Definition-local member path.
        path: Vec<&'static str>,
    },
    /// A generated display name is empty or aliases a different definition.
    NameCollision(String),
    /// A required resolved source target is missing.
    Missing(DefinitionId),
}
impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graph(error) => error.fmt(f),
            Self::Unsupported { path } => {
                write!(f, "unsupported selected containment at {}", path.join("."))
            }
            Self::NameCollision(name) => write!(f, "invalid or conflicting generated name {name}"),
            Self::Missing(id) => write!(f, "missing source definition {}", id.path()),
        }
    }
}
impl std::error::Error for TransformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            Self::Unsupported { .. } | Self::NameCollision(_) | Self::Missing(_) => None,
        }
    }
}
const fn all(_: &Occurrence) -> bool {
    true
}
fn name(site: &Occurrence) -> String {
    format!("{}_{}", site.owner_name, site.path.join("_"))
}
/// Lifts selected inline composite occurrences into reusable definitions.
#[derive(Debug)]
pub struct Outline<S = fn(&Occurrence) -> bool, N = fn(&Occurrence) -> String> {
    select: S,
    name: N,
}
impl Outline {
    /// Selects every inline reusable composite, using owner/member names.
    pub fn new() -> Self {
        Self { select: all, name }
    }
}
impl Default for Outline {
    fn default() -> Self {
        Self::new()
    }
}
impl<S, N> Outline<S, N> {
    /// Selects occurrences by definition-local path; scalar leaves are not candidates.
    pub fn select<T>(self, select: T) -> Outline<T, N>
    where
        T: FnMut(&Occurrence) -> bool,
    {
        Outline {
            select,
            name: self.name,
        }
    }
    /// Chooses display names independently of source-derived identity.
    pub fn name_with<T>(self, name: T) -> Outline<S, T>
    where
        T: FnMut(&Occurrence) -> String,
    {
        Outline {
            select: self.select,
            name,
        }
    }
}
impl<S, N> Pass<&SchemaGraph> for Outline<S, N>
where
    S: FnMut(&Occurrence) -> bool,
    N: FnMut(&Occurrence) -> String,
{
    type Output = SchemaSet;
    type Error = TransformError;
    fn run(self, graph: &SchemaGraph) -> Result<SchemaSet, Self::Error> {
        engine::Engine::new(graph, true, self.select, self.name).run()
    }
}
/// Expands selected references into owned source containment.
#[derive(Debug)]
pub struct Inline<S = fn(&Occurrence) -> bool> {
    select: S,
}
impl Inline {
    /// Selects every reference having a supported containment representation.
    pub fn new() -> Self {
        Self { select: all }
    }
}
impl Default for Inline {
    fn default() -> Self {
        Self::new()
    }
}
impl<S> Inline<S> {
    /// Selects reference occurrences in their declaring definition's scope.
    pub fn select<T>(self, select: T) -> Inline<T>
    where
        T: FnMut(&Occurrence) -> bool,
    {
        Inline { select }
    }
}
impl<S: FnMut(&Occurrence) -> bool> Pass<&SchemaGraph> for Inline<S> {
    type Output = SchemaSet;
    type Error = TransformError;
    fn run(self, graph: &SchemaGraph) -> Result<SchemaSet, Self::Error> {
        engine::Engine::new(graph, false, self.select, name).run()
    }
}

impl<S, N> Pass<SchemaGraph> for Outline<S, N>
where
    S: FnMut(&Occurrence) -> bool,
    N: FnMut(&Occurrence) -> String,
{
    type Output = SchemaSet;
    type Error = TransformError;
    fn run(self, graph: SchemaGraph) -> Result<SchemaSet, Self::Error> {
        self.run(&graph)
    }
}
impl<S, N> Pass<SchemaSet> for Outline<S, N>
where
    S: FnMut(&Occurrence) -> bool,
    N: FnMut(&Occurrence) -> String,
{
    type Output = SchemaSet;
    type Error = TransformError;
    fn run(self, set: SchemaSet) -> Result<SchemaSet, Self::Error> {
        self.run(set.resolve().map_err(TransformError::Graph)?)
    }
}
impl<S: FnMut(&Occurrence) -> bool> Pass<SchemaGraph> for Inline<S> {
    type Output = SchemaSet;
    type Error = TransformError;
    fn run(self, graph: SchemaGraph) -> Result<SchemaSet, Self::Error> {
        self.run(&graph)
    }
}
impl<S: FnMut(&Occurrence) -> bool> Pass<SchemaSet> for Inline<S> {
    type Output = SchemaSet;
    type Error = TransformError;
    fn run(self, set: SchemaSet) -> Result<SchemaSet, Self::Error> {
        self.run(set.resolve().map_err(TransformError::Graph)?)
    }
}
