use super::{Context, Edge, NodeId, Schema, Visit};
use crate::analysis::{AnalyzedSchema, Unit};
use std::rc::Rc;

/// Semantic domain nodes expose the same immutable containment operation.
pub trait DomainNode<'a>: Copy {
    /// Children in canonical source order, before any consumer-selected reordering.
    fn children(self) -> std::vec::IntoIter<Self>;
    /// Classifies a child edge before centrally assigning source-role ordinals.
    fn edge(self, index: usize, _child: Self) -> Edge {
        Edge::Child(index)
    }
}
/// Framework-owned node, identity and lexical ancestry shared by every domain.
/// ```compile_fail
/// use hw_model_schema::traversal::record::Context;
/// fn rebind(mut context: Context<'_>) { context.node = context.node(); }
/// ```
/// Roots can only be obtained from the framework's schema traversal:
/// ```compile_fail
/// use hw_model_schema::{analysis::{AnalyzedSchema, Unit}, ast::record::Node, traversal::record::Context};
/// fn forge<'a>(schema: &'a AnalyzedSchema, unit: &'a Unit, node: Node<'a>) {
///     let _ = Context::root(schema, unit, node);
/// }
/// ```
#[derive(Debug)]
pub struct BoundContext<'a, N, S: Schema = AnalyzedSchema> {
    schema: &'a S,
    owner: &'a S::Root,
    node: N,
    id: NodeId,
    ancestors: Rc<Vec<N>>,
}
impl<'a, N: DomainNode<'a>, S: Schema> BoundContext<'a, N, S> {
    pub(crate) fn root(schema: &'a S, owner: &'a S::Root, node: N) -> Self {
        Self {
            schema,
            owner,
            node,
            id: S::root_id(owner),
            ancestors: Rc::new(Vec::new()),
        }
    }
    /// Immutable source provider at this processing stage.
    pub const fn schema(&self) -> &'a S {
        self.schema
    }
    /// Actual selected root data, preserved through reference expansions.
    pub const fn owner(&self) -> &'a S::Root {
        self.owner
    }
    /// Current semantic node.
    pub const fn node(&self) -> N {
        self.node
    }
    /// Identity of this occurrence rather than merely its reusable definition.
    pub const fn id(&self) -> &NodeId {
        &self.id
    }
    /// Identity of the immediate containing occurrence.
    pub fn parent_id(&self) -> Option<NodeId> {
        self.id.parent()
    }
    /// Returns the actual containing occurrence, including a reference's use site.
    /// A selected root has no parent; this never changes the bound schema or owner.
    pub fn parent(&self) -> Option<Self> {
        let (node, ancestors) = self.ancestors.split_last()?;
        Some(Self {
            schema: self.schema,
            owner: self.owner,
            node: *node,
            id: self.id.parent()?,
            ancestors: Rc::new(ancestors.to_vec()),
        })
    }
    /// True containment ancestors in root-to-parent order.
    pub fn ancestors(&self) -> &[N] {
        &self.ancestors
    }
    /// Resolves an imported reference to its reusable root.
    pub fn resolve(&self, reference: &crate::ast::Reference) -> Option<&'a S::Root> {
        self.schema.definition(reference.target().id())
    }
    /// Current bound visit, without permitting node rebinding.
    pub fn current(&self) -> Visit<Self> {
        Visit::new(self.clone())
    }
    /// Children receive structural identities before consumer reordering.
    pub fn children(&self) -> std::vec::IntoIter<Visit<Self>> {
        let mut ordinals = std::collections::HashMap::new();
        self.node
            .children()
            .enumerate()
            .map(|(index, node)| {
                let edge = match self.node.edge(index, node) {
                    Edge::Source(role, _) => {
                        let ordinal = ordinals.entry(role).or_insert(0);
                        let edge = Edge::Source(role, *ordinal);
                        *ordinal += 1;
                        edge
                    }
                    edge => edge,
                };
                Visit::new(self.descendant(node, edge))
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
    pub(crate) fn descendant(&self, node: N, edge: Edge) -> Self {
        let mut ancestors = self.ancestors.as_ref().clone();
        ancestors.push(self.node);
        Self {
            schema: self.schema,
            owner: self.owner,
            node,
            id: self.id.child(edge),
            ancestors: Rc::new(ancestors),
        }
    }
    pub(crate) fn lineage(&self) -> impl Iterator<Item = N> + '_ {
        core::iter::once(self.node).chain(self.ancestors.iter().rev().copied())
    }
}
impl<'a, N: DomainNode<'a>, S: Schema> Context for BoundContext<'a, N, S> {
    type Node = N;
    fn node(&self) -> N {
        self.node()
    }
    fn id(&self) -> &NodeId {
        self.id()
    }
    fn current(&self) -> Visit<Self> {
        self.current()
    }
    fn children(&self) -> std::vec::IntoIter<Visit<Self>> {
        self.children()
    }
}

impl<N: Clone, S: Schema> Clone for BoundContext<'_, N, S> {
    fn clone(&self) -> Self {
        Self {
            schema: self.schema,
            owner: self.owner,
            node: self.node.clone(),
            id: self.id.clone(),
            ancestors: self.ancestors.clone(),
        }
    }
}
impl<'a, N> BoundContext<'a, N> {
    /// Completed semantic analysis available to analyzed passes and backends.
    pub const fn analysis(&self) -> &'a AnalyzedSchema {
        self.schema
    }
    /// Selected completed definition or projection.
    pub const fn unit(&self) -> &'a Unit {
        self.owner
    }
}
