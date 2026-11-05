/// Read-only bound traversal context implemented by the framework domains.
pub trait Context: Clone {
    /// Domain node type, exposed only for inspection.
    type Node: Copy;
    /// Current semantic node.
    fn node(&self) -> Self::Node;
    /// Stable identity of this semantic occurrence.
    fn id(&self) -> &super::NodeId;
    /// Identity of the containing occurrence, absent at roots.
    fn parent_id(&self) -> Option<super::NodeId> {
        self.id().parent()
    }
    /// Current bound visit.
    fn current(&self) -> Visit<Self>;
    /// Bound semantic children.
    fn children(&self) -> std::vec::IntoIter<Visit<Self>>;
}
/// A semantic node bound to its original lexical context.
///
/// Targets cannot manufacture a visit from a context:
/// ```compile_fail
/// use hw_model_schema::traversal::{record::Context as RecordContext, Visit};
/// fn rebind(ctx: RecordContext<'_>) { let _ = Visit::new(ctx); }
/// ```
/// A struct literal also cannot create an independently bound visit:
/// ```compile_fail
/// use hw_model_schema::traversal::{record::Context as RecordContext, Visit};
/// fn forge(ctx: RecordContext<'_>) { let _ = Visit { ctx }; }
/// ```
#[derive(Debug, Clone)]
pub struct Visit<S> {
    ctx: S,
}
impl<S: Context> Visit<S> {
    pub(crate) const fn new(ctx: S) -> Self {
        Self { ctx }
    }
    /// Inspects the semantic node without granting rebinding capability.
    pub fn node(&self) -> S::Node {
        self.ctx.node()
    }
    /// Inspects the immutable lexical context.
    pub const fn context(&self) -> &S {
        &self.ctx
    }
    pub(crate) fn into_context(self) -> S {
        self.ctx
    }
}
