use crate::ast::DefinitionId;

/// A containment edge in the canonical source order, independent of visit order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    /// Child ordinal in the immutable semantic tree.
    Child(usize),
    /// Source child ordinal within a semantic role, stable when derived siblings appear.
    Source(SourceRole, usize),
    /// Layout-derived child with no original source occurrence.
    Derived(usize),
    /// Expansion of a reference at this particular occurrence.
    Referent,
}
/// Structural source roles; ordinals preserve duplicate names as distinct nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceRole {
    /// Declaration contained by a selected root.
    Declaration,
    /// Constructor parameter.
    Parameter,
    /// Source placement.
    Placement,
    /// Constructor argument.
    Argument,
    /// Register field.
    Field,
    /// Source attribute identified by its schema spelling.
    Attribute(&'static str),
    /// Shared semantic value or expression operand.
    Value,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Root {
    Definition(DefinitionId),
    View(String),
}
/// Opaque identity of a semantic occurrence, including roots and reference expansions.
///
/// Identity consists of the imported root identity and a structural edge path. It is
/// stable under traversal reordering of the same immutable schema; it is not a
/// persistent identifier across source edits or different schema revisions. Named
/// views and reusable definitions occupy separate namespaces. Register source edges
/// retain their semantic-role ordinals when layout adds derived siblings; derived
/// expressions and instances have a disjoint edge namespace. Repeated references
/// have separate paths even when they resolve to the same reusable definition.
/// Only the framework constructs identities; consumers cannot rebind a visit.
/// ```compile_fail
/// use hw_model_schema::traversal::NodeId;
/// let id = NodeId::view("forged");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId {
    root: Root,
    path: Vec<Edge>,
}
impl NodeId {
    pub(crate) const fn definition(id: DefinitionId) -> Self {
        Self {
            root: Root::Definition(id),
            path: Vec::new(),
        }
    }
    pub(crate) fn view(name: &str) -> Self {
        Self {
            root: Root::View(name.into()),
            path: Vec::new(),
        }
    }
    pub(super) fn child(&self, edge: Edge) -> Self {
        let mut id = self.clone();
        id.path.push(edge);
        id
    }
    /// Imported root's Rust diagnostic path, absent for explicitly named views.
    pub const fn source_path(&self) -> Option<&'static str> {
        match &self.root {
            Root::Definition(id) => Some(id.path()),
            Root::View(_) => None,
        }
    }
    /// Explicit root view name, absent for reusable imported definitions.
    pub fn view_name(&self) -> Option<&str> {
        match &self.root {
            Root::Definition(_) => None,
            Root::View(name) => Some(name),
        }
    }
    /// Structural edges from the selected root to this occurrence.
    pub fn path(&self) -> &[Edge] {
        &self.path
    }
    /// Whether this identity names a selected reusable definition or named view.
    pub const fn is_root(&self) -> bool {
        self.path.is_empty()
    }
    /// The containing occurrence, absent at a root.
    pub fn parent(&self) -> Option<Self> {
        let mut id = self.clone();
        id.path.pop()?;
        Some(id)
    }
}
