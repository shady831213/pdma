use crate::ast::record::Node;
use crate::export::{Kind, Visit, context, view};
use crate::{Documentation, traversal::NodeId};

/// Renderer context exposing metadata and bound visits without the semantic tree.
#[derive(Clone)]
pub struct Context<'a>(pub(super) crate::traversal::record::Context<'a>);
impl context::sealed::Context for Context<'_> {}
impl<'a> Context<'a> {
    /// Selected root identity, preserved through reference expansion.
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
    /// Analyzed record size and alignment, when this root is a record.
    pub const fn root_extent(&self) -> Option<(usize, usize)> {
        match &self.0.unit().body {
            crate::analysis::Layout::Record(r) => Some((r.size(), r.alignment())),
            crate::analysis::Layout::None | crate::analysis::Layout::Map(_) => None,
        }
    }
    /// Stable identity of the current occurrence.
    pub const fn id(&self) -> &NodeId {
        self.0.id()
    }
    /// Identity of the immediately containing occurrence.
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
            Node::Record(_) => Kind::Record,
            Node::Union(_) => Kind::Union,
            Node::Field(_) => Kind::Field,
            Node::Case(_) => Kind::Case,
            Node::Reference(_) => Kind::Reference,
            Node::Constant(_) => Kind::Constant,
            Node::Attribute(_) => Kind::Attribute,
            Node::Value(v) => context::value_kind(v),
        }
    }
    /// Source name of the immediate lexical parent.
    pub fn parent_name(&self) -> Option<String> {
        self.0
            .parent()
            .and_then(|parent| crate::export::Context::name(&Self(parent)).map(str::to_owned))
    }
    /// Whether this is the selected root occurrence.
    pub const fn is_root(&self) -> bool {
        self.id().is_root()
    }
    /// A bound visit of the current occurrence.
    pub fn current(&self) -> Visit<Self> {
        Visit::new(self.clone())
    }
    /// Bound child visits in canonical semantic order.
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
        ) && let Some(record) = self.storage_schema()
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
    fn storage_schema(&self) -> Option<&'a crate::ResolvedRecord> {
        self.0.opaque_layout()
    }
    /// Enclosing opaque storage member without access to its child declarations.
    pub fn storage_member(&self) -> Option<view::StorageMember<'a>> {
        core::iter::once(self.0.node())
            .chain(self.0.ancestors().iter().rev().copied())
            .find_map(|node| match node {
                Node::Value(crate::ast::value::Node::StorageMember(member)) => {
                    Some(view::StorageMember(member))
                }
                _ => None,
            })
    }
    /// Opaque member path including the enclosing reference placement.
    pub fn storage_path(&self) -> Option<String> {
        if let Some(member) = self.storage_field() {
            let path = self.path();
            let parent = path.rsplit_once('.').map_or("", |(parent, _)| parent);
            return Some(if parent.is_empty() {
                member.name().into()
            } else {
                format!("{parent}.{}", member.name())
            });
        }
        let member = self.storage_member()?;
        let root = self.storage_schema()?.root();
        let relative = member
            .path()
            .strip_prefix(root)
            .and_then(|v| v.strip_prefix('.'))
            .unwrap_or_else(|| member.path());
        let path = self.path();
        Some(if path.is_empty() {
            relative.into()
        } else {
            format!("{path}.{relative}")
        })
    }
    /// Enclosing logical field as a shallow view.
    pub fn field(&self) -> Option<view::Field<'a>> {
        self.0.field().map(view::Field)
    }
    /// Enclosing bit-field source name.
    pub fn bit(&self) -> Option<&'static str> {
        self.0.bit()
    }
    /// Source field and case path relative to the selected root.
    pub fn path(&self) -> String {
        self.0
            .ancestors()
            .iter()
            .copied()
            .chain([self.0.node()])
            .filter_map(|n| match n {
                Node::Field(v) => Some(v.name()),
                Node::Case(v) => Some(v.name()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(".")
    }
    /// Source name of the containing record.
    pub fn record_name(&self) -> Option<&'static str> {
        self.0.ancestors().iter().find_map(|n| match n {
            Node::Record(v) => Some(v.spec().name()),
            _ => None,
        })
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
            Node::Record(v) => Some(v.spec().name()),
            Node::Union(v) => Some(v.storage().name()),
            Node::Field(v) => Some(v.name()),
            Node::Case(v) => Some(v.name()),
            Node::Reference(v) => Some(v.name()),
            Node::Constant(v) => Some(v.symbol),
            Node::Attribute(_) => None,
            Node::Value(v) => context::value_name(v),
        }
    }
    fn documentation(&self) -> Documentation {
        match self.0.node() {
            Node::Record(v) => v.spec().documentation(),
            Node::Union(v) => v.storage().documentation(),
            Node::Field(v) => v.documentation(),
            Node::Value(v) => context::value_documentation(v),
            Node::Case(_) | Node::Reference(_) | Node::Constant(_) | Node::Attribute(_) => {
                Documentation::EMPTY
            }
        }
    }
    fn children(&self) -> std::vec::IntoIter<Visit<Self>> {
        self.children()
    }
}

/// Binds selected roots for restricted renderer dispatch.
pub fn roots(schema: &crate::analysis::AnalyzedSchema) -> impl Iterator<Item = Visit<Context<'_>>> {
    crate::traversal::record::roots(schema).map(|v| Visit::new(Context(v.into_context())))
}

impl core::fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Context")
            .field("id", self.id())
            .field("kind", &crate::export::Context::kind(self))
            .finish()
    }
}
