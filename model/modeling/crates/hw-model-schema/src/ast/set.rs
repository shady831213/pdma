use std::collections::{HashMap, HashSet};

use super::{Definition, DefinitionBody, DefinitionId, DescribeSchema, LayoutError, MapInstance};

/// Failure while collecting a complete definition graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// A requested instance cannot be resolved from its map definition.
    InstanceLayout(LayoutError),
    /// Distinct configurations share one instance name, or that name is empty.
    InstanceName(String),
    /// A non-map definition was supplied to a map-instantiation operation.
    NotMap(&'static str),
    /// A by-value layout dependency returns to an active ancestor.
    Cycle(Vec<String>),
    /// Different definitions have the same canonical display path.
    AmbiguousPath(String),
}
impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstanceLayout(error) => error.fmt(f),
            Self::InstanceName(name) => write!(f, "invalid or conflicting instance name {name}"),
            Self::NotMap(path) => write!(f, "{path} is not a register-map definition"),
            Self::Cycle(path) => write!(f, "recursive schema dependency: {}", path.join(" -> ")),
            Self::AmbiguousPath(path) => write!(f, "distinct schema definitions share path {path}"),
        }
    }
}
impl std::error::Error for GraphError {}

/// Explicit roots populated manually, through discovery, or both.
#[derive(Debug, Default)]
pub struct SchemaSet {
    roots: Vec<Definition>,
    instances: Vec<MapInstance>,
}

impl SchemaSet {
    /// Creates an empty selection without requiring registration support.
    pub const fn new() -> Self {
        Self {
            roots: Vec::new(),
            instances: Vec::new(),
        }
    }
    /// Selects a record or union, including custom opaque layouts.
    pub fn add_record<T: crate::SchemaLayout + 'static>(&mut self) {
        self.add_definition(T::schema_definition());
    }
    /// Selects any generated or user-described schema definition.
    pub fn add<T: DescribeSchema>(&mut self) {
        self.add_definition(T::schema_definition());
    }
    /// Adds one root; selecting the same definition again is idempotent.
    pub fn add_definition(&mut self, definition: Definition) {
        if !self.roots.iter().any(|root| root.id() == definition.id()) {
            self.roots.push(definition);
        }
    }
    /// Combines independently selected sets without losing dependency provenance.
    pub fn extend(&mut self, other: Self) -> Result<(), GraphError> {
        for instance in &other.instances {
            self.check_instance(instance)?;
        }
        for definition in other.roots {
            self.add_definition(definition);
        }
        for instance in other.instances {
            if !self.instances.contains(&instance) {
                self.instances.push(instance);
            }
        }
        Ok(())
    }
    /// Selects a map and binds one named instance; definition and instance identities remain distinct.
    pub fn add_map_instance<T: DescribeSchema>(
        &mut self,
        name: &str,
        arguments: &[usize],
    ) -> Result<(), GraphError> {
        let definition = T::schema_definition();
        let DefinitionBody::RegisterMap(map) = definition.body() else {
            return Err(GraphError::NotMap(definition.id().path()));
        };
        let instance = MapInstance {
            name: name.to_owned(),
            definition: definition.id(),
            arguments: arguments.to_vec(),
            geometry: map.resolve(arguments).map_err(GraphError::InstanceLayout)?,
        };
        self.check_instance(&instance)?;
        if !self.instances.contains(&instance) {
            self.instances.push(instance);
        }
        self.add_definition(definition);
        Ok(())
    }
    fn check_instance(&self, instance: &MapInstance) -> Result<(), GraphError> {
        if instance.name.is_empty()
            || self
                .instances
                .iter()
                .any(|old| old.name == instance.name && old != instance)
        {
            return Err(GraphError::InstanceName(instance.name.clone()));
        }
        Ok(())
    }
    /// Retains named instances when a source rewrite preserves root identities.
    pub(crate) fn with_instances(mut self, instances: &[MapInstance]) -> Self {
        self.instances = instances.to_vec();
        self
    }
    /// Collects dependencies and checks cycles and identity/path ambiguity.
    pub fn resolve(mut self) -> Result<SchemaGraph, GraphError> {
        for instance in &mut self.instances {
            let definition = self
                .roots
                .iter()
                .find(|root| root.id() == instance.definition)
                .ok_or_else(|| GraphError::NotMap(instance.definition.path()))?;
            let DefinitionBody::RegisterMap(map) = definition.body() else {
                return Err(GraphError::NotMap(instance.definition.path()));
            };
            instance.geometry = map
                .resolve(&instance.arguments)
                .map_err(GraphError::InstanceLayout)?;
        }
        self.roots
            .sort_by_key(|node| (node.id().path(), node.id().structural_path().to_vec()));
        self.instances
            .sort_by(|left, right| left.name.cmp(&right.name));
        let roots = self.roots.iter().map(Definition::id).collect();
        let mut collector = Collector::default();
        for root in self.roots {
            collector.visit(root, &mut Vec::new())?;
        }
        let mut definitions: Vec<_> = collector.complete.into_values().collect();
        definitions.sort_by_key(|node| (node.id().path(), node.id().structural_path().to_vec()));
        Ok(SchemaGraph {
            definitions,
            roots,
            order: collector.order,
            instances: self.instances,
        })
    }
}

/// Validated graph with stable traversal order and explicit root provenance.
#[derive(Debug)]
pub struct SchemaGraph {
    definitions: Vec<Definition>,
    roots: Vec<DefinitionId>,
    order: Vec<DefinitionId>,
    instances: Vec<MapInstance>,
}
impl SchemaGraph {
    /// Returns named, checked map configurations in name order.
    pub fn instances(&self) -> &[MapInstance] {
        &self.instances
    }
    /// Iterates the validated graph with each dependency before its users.
    pub fn dependency_order(&self) -> impl Iterator<Item = &Definition> {
        self.order.iter().filter_map(|id| self.get(id.clone()))
    }
    /// Iterates unique definitions in canonical Rust-path order.
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.definitions.iter()
    }
    /// Returns selected roots, excluding automatically reached dependencies.
    pub fn roots(&self) -> &[DefinitionId] {
        &self.roots
    }
    /// Finds a definition using identity rather than its short name.
    pub fn get(&self, id: impl std::borrow::Borrow<DefinitionId>) -> Option<&Definition> {
        self.definitions
            .iter()
            .find(|node| &node.id() == id.borrow())
    }
}

#[derive(Default)]
struct Collector {
    order: Vec<DefinitionId>,
    complete: HashMap<DefinitionId, Definition>,
    active: HashSet<DefinitionId>,
    paths: HashMap<(&'static str, Vec<String>), DefinitionId>,
}
impl Collector {
    fn visit(&mut self, definition: Definition, path: &mut Vec<String>) -> Result<(), GraphError> {
        let id = definition.id();
        if self.complete.contains_key(&id) {
            return Ok(());
        }
        if !self.active.insert(id.clone()) {
            let mut cycle = path.clone();
            cycle.push(id.path().to_owned());
            return Err(GraphError::Cycle(cycle));
        }
        if self
            .paths
            .insert((id.path(), id.structural_path().to_vec()), id.clone())
            .is_some_and(|previous| previous != id)
        {
            return Err(GraphError::AmbiguousPath(id.path().to_owned()));
        }
        for reference in definition.references() {
            path.push(format!("{}.{}", id.path(), reference.name()));
            self.visit(reference.target(), path)?;
            path.pop();
        }
        self.active.remove(&id);
        self.complete.insert(id.clone(), definition);
        self.order.push(id);
        Ok(())
    }
}
