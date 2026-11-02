use super::super::value::{Context, Visit};
use crate::analysis::{AnalyzedSchema, Unit};
use crate::ast::record::Node;
/// Framework-owned lexical context. Its node and ancestry cannot be rebound.
/// ```compile_fail
/// use hw_model_schema::export::record::EmitContext;
/// fn forge(mut ctx: EmitContext<'_>) { ctx.node = ctx.node(); }
/// ```
/// Root construction belongs to the framework:
/// ```compile_fail
/// use hw_model_schema::{analysis::{AnalyzedSchema, Unit}, ast::record::Node, export::record::EmitContext};
/// fn root<'a>(a: &'a AnalyzedSchema, u: &'a Unit, n: Node<'a>) {
///     let _ = EmitContext::root(a, u, n);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EmitContext<'a> {
    analysis: &'a AnalyzedSchema,
    unit: &'a Unit,
    node: Node<'a>,
    ancestors: std::rc::Rc<Vec<Node<'a>>>,
}
impl<'a> EmitContext<'a> {
    pub(super) fn root(analysis: &'a AnalyzedSchema, unit: &'a Unit, node: Node<'a>) -> Self {
        Self {
            analysis,
            unit,
            node,
            ancestors: std::rc::Rc::new(Vec::new()),
        }
    }
    /// Completed semantic analysis, available read-only to targets.
    pub const fn analysis(&self) -> &'a AnalyzedSchema {
        self.analysis
    }
    /// Selected definition or projection.
    pub const fn unit(&self) -> &'a Unit {
        self.unit
    }
    /// Resolves a source reference against the completed definition graph.
    pub fn resolve(&self, reference: &crate::ast::Reference) -> Option<&'a Unit> {
        let id = crate::analysis::UnitId::Definition(reference.target().id());
        self.analysis.units().find(|unit| unit.id == id)
    }
    /// Current semantic node.
    pub const fn node(&self) -> Node<'a> {
        self.node
    }
    /// True containment ancestors in root-to-parent order.
    pub fn ancestors(&self) -> &[Node<'a>] {
        &self.ancestors
    }
    /// Bound current node for dispatch.
    pub fn current(&self) -> Visit<Self> {
        Visit::new(self.clone())
    }
    /// Bound children, independently reorderable without changing lexical parents.
    pub fn children(&self) -> std::vec::IntoIter<Visit<Self>> {
        let mut ancestors = self.ancestors.as_ref().clone();
        ancestors.push(self.node);
        let ancestors = std::rc::Rc::new(ancestors);
        self.node
            .children()
            .map(|node| {
                Visit::new(Self {
                    analysis: self.analysis,
                    unit: self.unit,
                    node,
                    ancestors: ancestors.clone(),
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
    fn lineage(&self) -> impl Iterator<Item = Node<'a>> + '_ {
        core::iter::once(self.node).chain(self.ancestors.iter().rev().copied())
    }
    /// Enclosing bit-field name.
    pub fn bit(&self) -> Option<&'static str> {
        self.lineage().find_map(|node| match node {
            Node::Value(crate::ast::value::Node::BitField(v)) => Some(v.name()),
            _ => None,
        })
    }
    /// Enclosing logical field.
    pub fn field(&self) -> Option<&'a crate::ast::Field> {
        self.lineage().find_map(|node| match node {
            Node::Field(v) => Some(v),
            _ => None,
        })
    }
}
impl<'a> Context for EmitContext<'a> {
    type Node = Node<'a>;
    fn node(&self) -> Self::Node {
        self.node()
    }
    fn current(&self) -> Visit<Self> {
        self.current()
    }
    fn children(&self) -> std::vec::IntoIter<Visit<Self>> {
        self.children()
    }
}
