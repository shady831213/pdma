use super::super::value::{Context, Visit};
use super::Node;
use crate::analysis::{
    AnalyzedSchema, ConcretePlacement, Layout, Placement, Unit, ViewPathSegment,
};
pub use crate::ast::register::LayoutPropertyKind;
use crate::ast::{LayoutExpr, RegisterLocation};
/// Framework-owned lexical context. Its node and ancestry cannot be rebound.
/// ```compile_fail
/// use hw_model_schema::export::register::EmitContext;
/// fn forge(mut ctx: EmitContext<'_>) { ctx.node = ctx.node(); }
/// ```
/// Root construction belongs to the framework:
/// ```compile_fail
/// use hw_model_schema::{analysis::{AnalyzedSchema, Unit}, export::register::{EmitContext, Node}};
/// fn root(a: &AnalyzedSchema, u: &Unit) { let _ = EmitContext::root(a, u, Node::Root(u)); }
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
            Node::RegisterField(v) => Some(v.name()),
            _ => None,
        })
    }
    /// Enclosing completed placement.
    pub fn placement(&self) -> Option<crate::analysis::Placement<'a>> {
        self.lineage().find_map(|node| match node {
            Node::Placement(v) => Some(crate::analysis::Placement::Symbolic(v)),
            Node::ConcretePlacement(v) => Some(crate::analysis::Placement::Concrete(v)),
            _ => None,
        })
    }
    /// Enclosing effective register declaration.
    pub fn register(&self) -> Option<&'a crate::ast::RegisterInfo> {
        self.lineage().find_map(|node| match node {
            Node::Register(v) => Some(v),
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

/// A completed layout property with its semantic owner and source expression.
#[derive(Debug, Clone, Copy)]
pub struct LayoutProperty<'a> {
    /// Relative placement owner; absent for a map-owned property.
    pub owner: Option<&'a str>,
    /// Stable property name independent of target spelling.
    pub name: &'static str,
    /// Completed expression.
    pub expression: &'a LayoutExpr,
    /// Semantic property classification.
    pub kind: LayoutPropertyKind,
}
/// Normalized register position independent of target naming conventions.
#[derive(Debug, Clone, Copy)]
pub struct RegisterSite<'a> {
    /// Root, standalone, template or instance position.
    pub location: RegisterLocation,
    /// Source path.
    pub path: &'a str,
    /// Structural path components, excluding the root.
    pub segments: &'a [ViewPathSegment],
    /// Offset in the enclosing map or bank template.
    pub offset: usize,
}
impl<'a> EmitContext<'a> {
    /// Active unbound map parameter names, regardless of traversal order.
    pub fn parameters(&self) -> &'a [&'static str] {
        match &self.unit().body {
            Layout::Map(map) => map.plan.as_ref().map_or(&[], |p| p.parameters()),
            Layout::None | Layout::Record(_) => &[],
        }
    }
    /// Original declared parameter names, including those bound by the root.
    pub fn declared_parameters(&self) -> &'a [&'static str] {
        match &self.unit().body {
            Layout::Map(map) => map.plan.as_ref().map_or(&[], |p| p.source_parameters()),
            Layout::None | Layout::Record(_) => &[],
        }
    }
    /// Active concrete root argument bindings, if present.
    pub fn bindings(&self) -> Option<&'a [usize]> {
        match &self.unit().body {
            Layout::Map(map) => map.plan.as_ref().and_then(|p| p.bindings()),
            Layout::None | Layout::Record(_) => None,
        }
    }
    /// Normalized position of the enclosing effective register.
    pub fn register_site(&self) -> Option<RegisterSite<'a>> {
        match self.placement() {
            Some(Placement::Concrete(ConcretePlacement::Register { view, location })) => {
                Some(RegisterSite {
                    location,
                    path: &view.path,
                    segments: &view.segments,
                    offset: view.offset,
                })
            }
            _ => self.register().map(|_| RegisterSite {
                location: RegisterLocation::Root,
                path: &self.unit().name,
                segments: &[],
                offset: 0,
            }),
        }
    }
    /// Structural register location.
    pub fn register_location(&self) -> RegisterLocation {
        self.register_site()
            .map_or(RegisterLocation::Root, |site| site.location)
    }
    /// Structural register segments, empty at a reusable root.
    pub fn register_segments(&self) -> &'a [ViewPathSegment] {
        self.register_site().map_or(&[], |site| site.segments)
    }
    /// Register source path or selected definition name.
    pub fn semantic_path(&self) -> &'a str {
        self.register_site()
            .map_or_else(|| self.unit().name.as_str(), |site| site.path)
    }
    /// Current completed layout property, with map extent owned by the map itself.
    pub fn layout_property(&self) -> Option<LayoutProperty<'a>> {
        let Node::LayoutProperty(property) = self.node() else {
            return None;
        };
        let owner = match self.placement() {
            Some(Placement::Symbolic(p)) => Some(p.reference.name()),
            _ => None,
        };
        let name = match property.kind {
            LayoutPropertyKind::Offset => "offset",
            LayoutPropertyKind::Count => "count",
            LayoutPropertyKind::Stride => "stride",
            LayoutPropertyKind::End => "end",
            LayoutPropertyKind::OccupiedEnd => "occupied_end",
            LayoutPropertyKind::Extent => "extent",
        };
        Some(LayoutProperty {
            owner,
            name,
            expression: property.expression,
            kind: property.kind,
        })
    }
}
