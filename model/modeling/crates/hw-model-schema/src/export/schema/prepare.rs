use super::{
    PreparedBody, PreparedDefinition, PreparedMap, PreparedReference, PreparedSchema,
    RegisterMapView,
};
use crate::{
    RecordGraphError,
    graph::{DefinitionBody, DefinitionId, DescribeSchema, LayoutError, SchemaGraph},
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
pub enum PreparationError {
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
impl std::fmt::Display for PreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
impl std::error::Error for PreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record { source, .. } => Some(source),
            Self::Layout(source) => Some(source),
            Self::Binding(_) | Self::ReferencedBinding(_) => None,
        }
    }
}
impl From<LayoutError> for PreparationError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

impl PreparedSchema {
    /// Resolves dependencies and layouts in the shared preparation layer.
    ///
    /// # Errors
    /// Rejects invalid layout/argument bindings. Bound roots must not also be used
    /// as generic dependencies in the same request; create a separate instance view.
    pub fn from_graph(graph: &SchemaGraph, bindings: &Bindings) -> Result<Self, PreparationError> {
        for id in bindings.values.keys() {
            if !graph.roots().contains(id)
                || !graph
                    .get(*id)
                    .is_some_and(|d| matches!(d.body(), DefinitionBody::RegisterMap(_)))
            {
                return Err(PreparationError::Binding(*id));
            }
            if graph
                .definitions()
                .any(|d| d.references().iter().any(|r| r.target().id() == *id))
            {
                return Err(PreparationError::ReferencedBinding(*id));
            }
        }
        let mut definitions: Vec<PreparedDefinition> = graph
            .dependency_order()
            .map(|definition| prepare_definition(definition, bindings))
            .collect::<Result<_, PreparationError>>()?;
        for instance in graph.instances() {
            let definition = graph
                .get(instance.definition())
                .ok_or_else(|| PreparationError::Binding(instance.definition()))?;
            let DefinitionBody::RegisterMap(map) = definition.body() else {
                return Err(PreparationError::Binding(instance.definition()));
            };
            definitions.push(PreparedDefinition {
                kind: super::SchemaKind::RegisterMap,
                id: instance.name().into(),
                name: instance.name().into(),
                documentation: definition.documentation(),
                references: vec![PreparedReference {
                    name: "definition",
                    kind: crate::graph::ReferenceKind::Placement,
                    target: definition.id(),
                    discriminant: None,
                }],
                body: PreparedBody::Map(PreparedMap {
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
    definition: &crate::graph::Definition,
    bindings: &Bindings,
) -> Result<PreparedDefinition, PreparationError> {
    let kind = match definition.body() {
        DefinitionBody::Bits(_) => super::SchemaKind::Bits,
        DefinitionBody::Register(_) => super::SchemaKind::Register,
        DefinitionBody::RegisterMap(_) => super::SchemaKind::RegisterMap,
        DefinitionBody::Record(_) => super::SchemaKind::Record,
        DefinitionBody::Union { selector, .. } => super::SchemaKind::Union {
            selector: *selector,
        },
        DefinitionBody::Opaque(_) => super::SchemaKind::Opaque,
    };
    let body = match definition.body() {
        DefinitionBody::Bits(bits) => PreparedBody::Bits(bits.clone()),
        DefinitionBody::Register(register) => PreparedBody::Register(register.clone()),
        DefinitionBody::Record(spec)
        | DefinitionBody::Opaque(spec)
        | DefinitionBody::Union { storage: spec, .. } => {
            PreparedBody::Record(spec.resolve().map_err(|source| PreparationError::Record {
                path: definition.id().path(),
                source,
            })?)
        }
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
            PreparedBody::Map(PreparedMap {
                plan: Some(plan),
                instance,
            })
        }
    };
    Ok(PreparedDefinition {
        kind,
        id: definition.id().into(),
        name: definition.name().into(),
        documentation: definition.documentation(),
        body,
        references: definition
            .references()
            .iter()
            .map(|r| PreparedReference {
                name: r.name(),
                kind: r.kind(),
                target: r.target().id(),
                discriminant: r.discriminant(),
            })
            .collect(),
    })
}
