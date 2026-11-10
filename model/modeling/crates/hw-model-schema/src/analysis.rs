//! Target-independent, prepared schemas consumed by all renderers.
mod complete;
mod instance;
mod math;
mod simplify;
mod storage;
pub use complete::{CompleteMapLayout, CompletePlacementLayout, Constraint};
mod input;
mod layout;
mod map;
mod pass;
mod pipeline;
pub use pass::{Pass, Pipeline};
mod stages;
pub use stages::{LaidOutSchema, LayoutSchema, Resolve, Simplify, Validate, ValidatedSchema};
/// Resolves graph identities and root selection.
pub mod resolve;

/// Validates source declarations before geometry construction.
pub mod validate;
use crate::{
    Documentation, ResolvedRecord,
    ast::{Node, Reference, ReferenceKind, SchemaSet},
};
pub use input::{AnalysisError, Bindings};
pub use map::{BankView, RegisterMapView, RegisterView, ViewPathSegment};

/// One prepared dependency, retaining the original relationship and wire tag.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Name of the referring member.
    pub name: &'static str,
    /// Relationship to the target definition.
    pub kind: ReferenceKind,
    /// Referenced type identity.
    pub target: crate::traversal::NodeId,
    /// Union wire discriminant, when present.
    pub discriminant: Option<u64>,
    /// External union selector interpreted in this edge's lexical owner scope.
    pub selector_binding: Option<&'static str>,
}
impl From<&Reference> for Dependency {
    fn from(reference: &Reference) -> Self {
        Self {
            name: reference.name(),
            kind: reference.kind(),
            target: crate::traversal::NodeId::definition(reference.target().id()),
            discriminant: reference.discriminant(),
            selector_binding: reference.selector_binding(),
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
    /// Original declaration form; independent from generation entry selection.
    pub origin: crate::ast::Origin,
    /// Explicitly selected generation entry, including referenced definitions.
    pub selected: bool,
    /// Canonical declaration; absent only for a concrete permission projection.
    pub declaration: Option<crate::ast::DefinitionBody>,
    /// Stable identity within a generation request.
    pub id: crate::traversal::NodeId,
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
    opaque_layouts: std::collections::HashMap<crate::traversal::NodeId, crate::ResolvedRecord>,
    union_storage:
        std::collections::HashMap<crate::traversal::NodeId, Vec<(usize, crate::FieldSpec)>>,
}
impl AnalyzedSchema {
    /// Looks up a reusable root at the Rust frontend identity boundary.
    pub fn definition(&self, id: crate::ast::DefinitionId) -> Option<&Unit> {
        let id = crate::traversal::NodeId::definition(id);
        self.units.iter().find(|unit| unit.id == id)
    }
    /// Looks up an explicitly named instance or permission projection root.
    pub fn view(&self, name: &str) -> Option<&Unit> {
        let id = crate::traversal::NodeId::view(name);
        self.units.iter().find(|unit| unit.id == id)
    }
    /// Iterates analyzed definitions and views in generation order.
    pub fn units(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter()
    }
    /// Selects an explicit source record with its dependencies and checks their layouts.
    pub fn record(name: &str, record: crate::ast::RecordDefinition) -> Result<Self, AnalysisError> {
        struct ManualRecord;
        let id = crate::ast::DefinitionId::of::<ManualRecord>().derived(&[name.into()]);
        let definition = crate::ast::Definition::from_body(
            id.clone(),
            name.into(),
            record.spec().documentation(),
            crate::ast::DefinitionBody::Record(record),
        );
        let mut set = SchemaSet::new();
        set.add_definition(definition);
        let graph = set.resolve().map_err(AnalysisError::Dependencies)?;
        let mut units = pipeline::prepare_graph(&graph, &Bindings::new())?;
        for unit in &mut units {
            if unit.id == crate::traversal::NodeId::definition(id.clone()) {
                unit.id = crate::traversal::NodeId::view(name);
            }
        }
        Self::finish(units)
    }
    /// Selects manually supplied storage metadata without inventing source fields.
    pub fn opaque(name: &str, spec: crate::RecordSpec) -> Result<Self, AnalysisError> {
        let layout = spec.resolve().map_err(|source| AnalysisError::Record {
            path: spec.name(),
            source,
        })?;
        Self::finish(vec![Unit {
            origin: crate::ast::Origin::Named,
            selected: true,
            declaration: Some(crate::ast::DefinitionBody::Opaque(spec)),
            id: crate::traversal::NodeId::view(name),
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
