use super::{Analysis, Dependency, Layout, MapLayout, RegisterMapView, Unit};
use crate::{
    RecordGraphError,
    ast::{DefinitionBody, DefinitionId, DescribeSchema, LayoutError, SchemaGraph},
};
use std::collections::HashMap;

/// Generator-supplied root instance parameters; child arguments follow schema references.
#[derive(Debug, Clone, Default)]
pub struct Bindings {
    values: HashMap<DefinitionId, Vec<usize>>,
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
            Self::SourceField { record, field } => {
                write!(f, "invalid source field {record}.{field}")
            }
            Self::MissingDeclaration(name) => write!(f, "missing declaration for {name}"),
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
            Self::Record { source, .. } => Some(source),
            Self::Layout(source) => Some(source),
            Self::Binding(_)
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

impl Analysis {
    /// Resolves dependencies and layouts in the shared preparation layer.
    ///
    /// # Errors
    /// Rejects invalid layout/argument bindings. Bound roots must not also be used
    /// as generic dependencies in the same request; create a separate instance view.
    pub fn from_graph(graph: &SchemaGraph, bindings: &Bindings) -> Result<Self, AnalysisError> {
        for id in bindings.values.keys() {
            if !graph.roots().contains(id)
                || !graph
                    .get(*id)
                    .is_some_and(|d| matches!(d.body(), DefinitionBody::RegisterMap(_)))
            {
                return Err(AnalysisError::Binding(*id));
            }
            if graph
                .definitions()
                .any(|d| d.references().iter().any(|r| r.target().id() == *id))
            {
                return Err(AnalysisError::ReferencedBinding(*id));
            }
        }
        let mut definitions: Vec<Unit> = graph
            .dependency_order()
            .map(|definition| prepare_definition(definition, bindings))
            .collect::<Result<_, AnalysisError>>()?;
        for instance in graph.instances() {
            let definition = graph
                .get(instance.definition())
                .ok_or_else(|| AnalysisError::Binding(instance.definition()))?;
            let DefinitionBody::RegisterMap(map) = definition.body() else {
                return Err(AnalysisError::Binding(instance.definition()));
            };
            definitions.push(Unit {
                declaration: Some(definition.body().clone()),
                id: instance.name().into(),
                name: instance.name().into(),
                documentation: definition.documentation(),
                references: vec![Dependency {
                    name: "definition",
                    kind: crate::ast::ReferenceKind::Placement,
                    target: definition.id(),
                    discriminant: None,
                }],
                body: Layout::Map(MapLayout {
                    plan: Some(map.layout_plan()?.bind(instance.arguments())?),
                    instance: Some(RegisterMapView::from_definition(
                        instance.name(),
                        definition.documentation(),
                        map,
                        instance.arguments(),
                    )?),
                }),
            });
        }
        Ok(Self { definitions })
    }
}

fn prepare_definition(
    definition: &crate::ast::Definition,
    bindings: &Bindings,
) -> Result<Unit, AnalysisError> {
    super::validate::node(definition.node())?;
    let body = match definition.body() {
        DefinitionBody::Bits(_) | DefinitionBody::Register(_) => Layout::None,
        DefinitionBody::Record(record) => resolve_record(record.spec(), definition.id())?,
        DefinitionBody::Opaque(spec) => resolve_record(*spec, definition.id())?,
        DefinitionBody::Union(union) => resolve_record(union.storage(), definition.id())?,
        DefinitionBody::RegisterMap(map) => {
            let plan = map.layout_plan()?;
            let args = bindings
                .values
                .get(&definition.id())
                .map(Vec::as_slice)
                .or_else(|| map.parameters().is_empty().then_some(&[]));
            let instance = args
                .map(|args| {
                    RegisterMapView::from_definition(
                        definition.name(),
                        definition.documentation(),
                        map,
                        args,
                    )
                })
                .transpose()?;
            let plan = match args {
                Some(args) => plan.bind(args)?,
                None => plan,
            };
            Layout::Map(MapLayout {
                plan: Some(plan),
                instance,
            })
        }
    };
    Ok(Unit {
        declaration: Some(definition.body().clone()),
        id: definition.id().into(),
        name: definition.name().into(),
        documentation: definition.documentation(),
        body,
        references: definition
            .references()
            .iter()
            .map(|r| Dependency {
                name: r.name(),
                kind: r.kind(),
                target: r.target().id(),
                discriminant: r.discriminant(),
            })
            .collect(),
    })
}

fn resolve_record(spec: crate::RecordSpec, id: DefinitionId) -> Result<Layout, AnalysisError> {
    spec.resolve()
        .map(Layout::Record)
        .map_err(|source| AnalysisError::Record {
            path: id.path(),
            source,
        })
}
