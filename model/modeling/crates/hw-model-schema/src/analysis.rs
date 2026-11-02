//! Target-independent, prepared schemas consumed by all renderers.
mod instance;
mod placement;
pub use placement::{ConcretePlacement, Placement};
mod complete;
mod math;
mod simplify;
pub use complete::{CompleteMapLayout, CompletePlacementLayout, Constraint};
mod input;
mod layout;
mod map;
mod pipeline;
/// Resolves graph identities and root selection.
pub mod resolve;

/// Validates source declarations before geometry construction.
pub mod validate;
use crate::{
    Documentation, ResolvedRecord,
    ast::{DefinitionId, Node, Reference, ReferenceKind, SchemaSet},
};
pub use input::{AnalysisError, Bindings};
pub use map::{BankView, RegisterMapView, RegisterView, ViewPathSegment};

/// Identity of a reusable definition or an explicitly named export view.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnitId {
    /// A Rust schema definition, independent of output naming.
    Definition(DefinitionId),
    /// A generator-owned resolved view.
    View(String),
}
impl From<DefinitionId> for UnitId {
    fn from(id: DefinitionId) -> Self {
        Self::Definition(id)
    }
}
impl From<&str> for UnitId {
    fn from(name: &str) -> Self {
        Self::View(name.into())
    }
}

/// One prepared dependency, retaining the original relationship and wire tag.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Name of the referring member.
    pub name: &'static str,
    /// Relationship to the target definition.
    pub kind: ReferenceKind,
    /// Referenced type identity.
    pub target: DefinitionId,
    /// Union wire discriminant, when present.
    pub discriminant: Option<u64>,
}
impl From<&Reference> for Dependency {
    fn from(reference: &Reference) -> Self {
        Self {
            name: reference.name(),
            kind: reference.kind(),
            target: reference.target().id(),
            discriminant: reference.discriminant(),
        }
    }
}

/// Complete layout and optional concrete instance of the same register map.
#[derive(Debug, Clone)]
pub struct MapLayout {
    /// Analyzed layout computation; absent for supplied resolved permission views.
    pub plan: Option<Box<CompleteMapLayout>>,
    /// Concrete root arguments, retaining bindings beside the source declaration.
    pub arguments: Option<Vec<usize>>,
    /// Concrete metadata when all parameters are bound, or a resolved view is supplied.
    pub instance: Option<RegisterMapView>,
}

/// Additional checked layout information attached to a canonical declaration.
#[derive(Debug, Clone)]
pub enum Layout {
    /// The declaration already contains all required scalar layout attributes.
    None,
    /// Resolved record, union storage or opaque byte layout.
    Record(ResolvedRecord),
    /// Symbolic or bound register-map layout.
    Map(MapLayout),
}

/// A prepared definition and its dependencies, before target naming.
#[derive(Debug, Clone)]
pub struct Unit {
    /// Canonical declaration; absent only for a concrete permission projection.
    pub declaration: Option<crate::ast::DefinitionBody>,
    /// Stable identity within a generation request.
    pub id: UnitId,
    /// Declared name or explicit view name.
    pub name: String,
    /// Definition documentation.
    pub documentation: Documentation,
    /// Original edges, including repeated references.
    pub references: Vec<Dependency>,
    /// Complete rendering input.
    pub body: Layout,
}

/// Completed units in dependency order, constructed only by successful analysis.
///
/// Unchecked units cannot be inserted into the completed input:
/// ```compile_fail
/// use hw_model_schema::analysis::AnalyzedSchema;
/// let unchecked = AnalyzedSchema { units: Vec::new() };
/// ```
#[derive(Debug, Clone)]
pub struct AnalyzedSchema {
    units: Vec<Unit>,
}
impl AnalyzedSchema {
    /// Iterates analyzed definitions and views in generation order.
    pub fn units(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter()
    }
    /// Selects an explicit source record with its dependencies and checks their layouts.
    pub fn record(name: &str, record: crate::ast::RecordDefinition) -> Result<Self, AnalysisError> {
        validate::record(&record)?;
        let layout = record
            .spec()
            .resolve()
            .map_err(|source| AnalysisError::Record {
                path: record.spec().name(),
                source,
            })?;
        let mut dependencies = SchemaSet::new();
        let mut references = Vec::new();
        for reference in Node::Record(crate::ast::record::Node::Record(&record)).references() {
            dependencies.add_definition(reference.target());
            references.push(Dependency::from(reference));
        }
        let graph = dependencies
            .resolve()
            .map_err(AnalysisError::Dependencies)?;
        let mut units = pipeline::prepare_graph(&graph, &Bindings::new())?;
        units.push(Unit {
            declaration: Some(crate::ast::DefinitionBody::Record(record)),
            id: name.into(),
            name: name.into(),
            documentation: layout.documentation(),
            references,
            body: Layout::Record(layout),
        });
        Self::finish(units)
    }
    /// Selects manually supplied storage metadata without inventing source fields.
    pub fn opaque(name: &str, spec: crate::RecordSpec) -> Result<Self, AnalysisError> {
        let layout = spec.resolve().map_err(|source| AnalysisError::Record {
            path: spec.name(),
            source,
        })?;
        Self::finish(vec![Unit {
            declaration: Some(crate::ast::DefinitionBody::Opaque(spec)),
            id: name.into(),
            name: name.into(),
            documentation: spec.documentation(),
            references: Vec::new(),
            body: Layout::Record(layout),
        }])
    }
}

impl Unit {
    /// Borrows the declaration or concrete projected map as the same schema node kind.
    pub fn node(&self) -> Result<crate::ast::Node<'_>, AnalysisError> {
        if let Some(source) = &self.declaration {
            if let (
                crate::ast::DefinitionBody::RegisterMap(declaration),
                Layout::Map(MapLayout {
                    plan: Some(plan),
                    arguments,
                    ..
                }),
            ) = (source, &self.body)
            {
                return Ok(Node::Register(crate::ast::register::Node::RegisterMap(
                    crate::ast::MapView::Analyzed {
                        declaration,
                        plan,
                        arguments: arguments.as_deref(),
                    },
                )));
            }
            return Ok(source.node());
        }
        if let Layout::Map(MapLayout {
            instance: Some(view),
            ..
        }) = &self.body
        {
            return Ok(Node::Register(crate::ast::register::Node::RegisterMap(
                crate::ast::MapView::Resolved(view),
            )));
        }
        Err(AnalysisError::MissingDeclaration(self.name.clone()))
    }
}
