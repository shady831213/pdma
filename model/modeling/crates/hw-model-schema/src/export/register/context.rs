use crate::export::{Kind, Visit, context, view};
use crate::traversal::register::Node;
use crate::{Documentation, traversal::NodeId};

/// Renderer context exposing metadata and bound visits without raw semantic trees.
#[derive(Clone)]
pub struct Context<'a>(pub(super) crate::traversal::register::Context<'a>);
impl context::sealed::Context for Context<'_> {}
impl<'a> Context<'a> {
    /// Selected root identity.
    pub const fn root_id(&self) -> &NodeId {
        &self.0.unit().id
    }
    /// Selected root source name.
    pub fn root_name(&self) -> &'a str {
        &self.0.unit().name
    }
    /// Selected root documentation.
    pub const fn root_documentation(&self) -> Documentation {
        self.0.unit().documentation
    }
    /// Stable occurrence identity.
    pub const fn id(&self) -> &NodeId {
        self.0.id()
    }
    /// Immediate containing occurrence identity.
    pub fn parent_id(&self) -> Option<NodeId> {
        self.0.parent_id()
    }
    /// Shallow classification of the immediate lexical parent.
    pub fn parent_kind(&self) -> Option<Kind> {
        self.0
            .parent()
            .map(|parent| crate::export::Context::kind(&Self(parent)))
    }
    /// Ancestor kinds from the root to the immediate parent, without raw node access.
    pub fn ancestor_kinds(&self) -> impl Iterator<Item = Kind> + '_ {
        self.0.ancestors().iter().copied().map(Self::node_kind)
    }
    const fn node_kind(node: Node<'_>) -> Kind {
        match node {
            Node::Root(_) => Kind::Root,
            Node::MapLayout(_) => Kind::MapLayout,
            Node::RegisterMap(_) => Kind::RegisterMap,
            Node::Register(_) => Kind::Register,
            Node::RegisterField(_) => Kind::RegisterField,
            Node::Placement(_) => Kind::Placement,
            Node::Reference(_) => Kind::Reference,
            Node::RegisterPlacement { .. } => Kind::RegisterPlacement,
            Node::Bank(_) => Kind::Bank,
            Node::BankProperty(_) => Kind::BankProperty,
            Node::Parameter(_) => Kind::Parameter,
            Node::Argument(_) => Kind::Argument,
            Node::Attribute(_) => Kind::Attribute,
            Node::LayoutProperty(_) => Kind::LayoutProperty,
            Node::Value(v) => context::value_kind(v),
        }
    }
    /// Source name of the immediate lexical parent.
    pub fn parent_name(&self) -> Option<String> {
        self.0
            .parent()
            .and_then(|parent| crate::export::Context::name(&Self(parent)).map(str::to_owned))
    }
    /// Whether this occurrence is the selected root.
    pub const fn is_root(&self) -> bool {
        self.id().is_root()
    }
    /// Repeats the bound occurrence.
    pub fn current(&self) -> Visit<Self> {
        Visit::new(self.clone())
    }
    /// Bound child visits in canonical order.
    pub fn children(&self) -> std::vec::IntoIter<Visit<Self>> {
        if matches!(
            self.0.node(),
            Node::Value(crate::ast::value::Node::Expression(_))
        ) {
            return Vec::new().into_iter();
        }
        if matches!(
            self.0.node(),
            Node::Value(crate::ast::value::Node::Opaque(_))
        ) && let Some(record) = self.0.opaque_layout()
        {
            return record
                .members()
                .iter()
                .enumerate()
                .map(|(index, member)| {
                    let node = Node::Value(crate::ast::value::Node::StorageMember(member));
                    Visit::new(Self(
                        self.0
                            .descendant(node, crate::traversal::Edge::Derived(index)),
                    ))
                })
                .collect::<Vec<_>>()
                .into_iter();
        }
        self.0
            .children()
            .map(|v| Visit::new(Self(v.into_context())))
            .collect::<Vec<_>>()
            .into_iter()
    }
    /// Enclosing bit-field source name.
    pub fn bit(&self) -> Option<&'static str> {
        self.0.bit()
    }
    /// Enclosing bank metadata.
    pub fn bank(&self) -> Option<view::Bank<'a>> {
        self.0.bank().map(view::Bank)
    }
    /// Enclosing effective register metadata.
    pub fn register(&self) -> Option<view::Register<'a>> {
        self.0.register().map(view::Register)
    }
    /// Whether documentation belongs to a repeated occurrence.
    pub fn repeated_documentation(&self) -> bool {
        self.0.repeated_documentation()
    }
    /// Unbound root parameter names.
    pub fn parameters(&self) -> &'a [&'static str] {
        self.0.parameters()
    }
    /// Declared root parameter names including bound parameters.
    pub fn declared_parameters(&self) -> &'a [&'static str] {
        self.0.declared_parameters()
    }
    /// Concrete root argument values.
    pub fn bindings(&self) -> Option<&'a [usize]> {
        self.0.bindings()
    }
    /// Scalar and path metadata for the enclosing register.
    pub fn register_site(&self) -> Option<crate::traversal::register::RegisterSite<'a>> {
        self.0.register_site()
    }
    /// Structural register location.
    pub fn register_location(&self) -> crate::ast::RegisterLocation {
        self.0.register_location()
    }
    /// Structural register path components.
    pub fn register_segments(&self) -> &'a [crate::analysis::ViewPathSegment] {
        self.0.register_segments()
    }
    /// Register source path or selected root name.
    pub fn semantic_path(&self) -> &'a str {
        self.0.semantic_path()
    }
    /// Current property with an opaque expression.
    pub fn layout_property(&self) -> Option<view::LayoutProperty<'a>> {
        self.0.layout_property().map(view::LayoutProperty)
    }
    /// Bound parameter visits from the enclosing map regardless of traversal order.
    pub fn lexical_parameters(&self) -> std::vec::IntoIter<Visit<Self>> {
        let mut owner = self.0.clone();
        while !matches!(owner.node(), Node::MapLayout(_)) {
            let Some(parent) = owner.parent() else {
                return Vec::new().into_iter();
            };
            owner = parent;
        }
        owner
            .children()
            .filter(|v| matches!(v.node(), Node::Parameter(_)))
            .map(|v| Visit::new(Self(v.into_context())))
            .collect::<Vec<_>>()
            .into_iter()
    }
}
impl crate::export::Context for Context<'_> {
    fn root_kind(&self) -> Kind {
        context::root_kind(self.0.unit())
    }
    fn id(&self) -> &NodeId {
        self.id()
    }
    fn kind(&self) -> Kind {
        Self::node_kind(self.0.node())
    }
    fn name(&self) -> Option<&str> {
        match self.0.node() {
            Node::Root(v) => Some(&v.name),
            Node::Register(v) => Some(v.name()),
            Node::RegisterField(v) => Some(v.name()),
            Node::Placement(v) => Some(v.source.name()),
            Node::Reference(v) => Some(v.name()),
            Node::RegisterPlacement { view, .. } => Some(view.name),
            Node::Bank(v) => Some(v.name()),
            Node::Parameter(v) => Some(v.name),
            Node::Argument(v) => v.parameter,
            Node::Value(v) => context::value_name(v),
            Node::MapLayout(_)
            | Node::RegisterMap(_)
            | Node::BankProperty(_)
            | Node::Attribute(_)
            | Node::LayoutProperty(_) => None,
        }
    }
    fn documentation(&self) -> Documentation {
        match self.0.node() {
            Node::Root(v) => v.documentation,
            Node::Register(v) => v.documentation(),
            Node::RegisterField(v) => v.documentation(),
            Node::Placement(v) => v.documentation,
            Node::RegisterPlacement { view, .. } => view.documentation,
            Node::Bank(v) => v.documentation(),
            Node::Value(v) => context::value_documentation(v),
            Node::MapLayout(_)
            | Node::RegisterMap(_)
            | Node::BankProperty(_)
            | Node::Parameter(_)
            | Node::Reference(_)
            | Node::Argument(_)
            | Node::Attribute(_)
            | Node::LayoutProperty(_) => Documentation::EMPTY,
        }
    }
    fn children(&self) -> std::vec::IntoIter<Visit<Self>> {
        self.children()
    }
}
/// Binds roots for restricted renderer dispatch.
pub fn roots(schema: &crate::analysis::AnalyzedSchema) -> impl Iterator<Item = Visit<Context<'_>>> {
    crate::traversal::register::roots(schema).map(|v| Visit::new(Context(v.into_context())))
}

impl core::fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Context")
            .field("id", self.id())
            .field("kind", &crate::export::Context::kind(self))
            .finish()
    }
}
