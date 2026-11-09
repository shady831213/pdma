//! Renderer capabilities; analysis traversal remains a separate API.
use crate::{Documentation, traversal::NodeId};

pub(super) mod sealed {
    pub trait Context {}
    pub trait Domain<B> {}
}

/// Shallow classification used to select and reorder bound visits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Selected register-domain root.
    Root,
    /// Logical record declaration.
    Record,
    /// Union declaration.
    Union,
    /// Logical record field.
    Field,
    /// Scalar value.
    Scalar,
    /// Reusable definition reference occurrence.
    Reference,
    /// Canonical physical storage field.
    StorageField,
    /// Union alternative.
    Case,
    /// Scalar constant constraint.
    Constant,
    /// Source attribute.
    Attribute,
    /// Bit container.
    Bits,
    /// Enumeration declaration.
    Enum,
    /// Bit field.
    BitField,
    /// Enumeration variant.
    Variant,
    /// Mathematical expression.
    Expression,
    /// Opaque storage envelope.
    Opaque,
    /// Register declaration.
    Register,
    /// Register bit field.
    RegisterField,
    /// Concrete map projection.
    RegisterMap,
    /// Completed symbolic map layout.
    MapLayout,
    /// Source placement geometry.
    Placement,
    /// Effective register placement.
    RegisterPlacement,
    /// Completed bank.
    Bank,
    /// Scalar bank property.
    BankProperty,
    /// Constructor parameter.
    Parameter,
    /// Child constructor argument.
    Argument,
    /// Completed layout property.
    LayoutProperty,
    /// Analyzed opaque storage member.
    StorageMember,
}

/// Framework-owned renderer context, without AST access.
pub trait Context: sealed::Context + Clone {
    /// Stable occurrence identity.
    fn id(&self) -> &NodeId;
    /// Shallow occurrence classification.
    fn kind(&self) -> Kind;
    /// Declaration category of the selected root, independent of this occurrence.
    fn root_kind(&self) -> Kind;
    /// Source name, if the occurrence is named.
    fn name(&self) -> Option<&str>;
    /// Source documentation of this occurrence.
    fn documentation(&self) -> Documentation;
    /// Visits in canonical containment order, retaining source attributes and IDs.
    /// Attribute visits emit no output; their shallow facts belong to owner views.
    /// Expressions are leaves; their operands belong to the shared semantic fold.
    fn children(&self) -> std::vec::IntoIter<Visit<Self>>;
    /// Repeats the current bound occurrence.
    fn current(&self) -> Visit<Self> {
        Visit::new(self.clone())
    }
    /// Immediate containing identity.
    fn parent_id(&self) -> Option<NodeId> {
        self.id().parent()
    }
}

/// Opaque occurrence retaining its original lexical binding.
#[derive(Debug, Clone)]
pub struct Visit<C> {
    ctx: C,
}
impl<C: Context> Visit<C> {
    pub(crate) const fn new(ctx: C) -> Self {
        Self { ctx }
    }
    pub(crate) fn into_context(self) -> C {
        self.ctx
    }
    pub(crate) const fn context(&self) -> &C {
        &self.ctx
    }
    /// Stable occurrence identity, unchanged by reorder or repetition.
    pub fn id(&self) -> &NodeId {
        self.ctx.id()
    }
    /// Immediate containing occurrence identity.
    pub fn parent_id(&self) -> Option<NodeId> {
        self.ctx.parent_id()
    }
    /// Shallow classification.
    pub fn kind(&self) -> Kind {
        self.ctx.kind()
    }
    /// Declaration category of the selected root.
    pub fn root_kind(&self) -> Kind {
        self.ctx.root_kind()
    }
    /// Source name when present.
    pub fn name(&self) -> Option<&str> {
        self.ctx.name()
    }
    /// Source documentation without exposing the referenced declaration.
    pub fn documentation(&self) -> Documentation {
        self.ctx.documentation()
    }
}

pub(super) const fn root_kind(unit: &crate::analysis::Unit) -> Kind {
    match unit.declaration.as_ref() {
        Some(crate::ast::DefinitionBody::Register(_)) => Kind::Register,
        Some(crate::ast::DefinitionBody::RegisterMap(_)) | None => Kind::RegisterMap,
        Some(crate::ast::DefinitionBody::Record(_)) => Kind::Record,
        Some(crate::ast::DefinitionBody::Union(_)) => Kind::Union,
        Some(crate::ast::DefinitionBody::Bits(_)) => Kind::Bits,
        Some(crate::ast::DefinitionBody::Opaque(_)) => Kind::Opaque,
    }
}

pub(super) const fn value_kind(node: crate::ast::value::Node<'_>) -> Kind {
    use crate::ast::value::Node;
    match node {
        Node::Scalar(_) => Kind::Scalar,
        Node::StorageField(_) => Kind::StorageField,
        Node::StorageMember(_) => Kind::StorageMember,
        Node::Opaque(_) => Kind::Opaque,
        Node::Bits(_) => Kind::Bits,
        Node::Enum(_) => Kind::Enum,
        Node::BitField(_) => Kind::BitField,
        Node::Variant(_) => Kind::Variant,
        Node::Expression(_) => Kind::Expression,
    }
}
pub(super) fn value_name(node: crate::ast::value::Node<'_>) -> Option<&str> {
    use crate::ast::value::Node;
    match node {
        Node::StorageField(v) => Some(v.name()),
        Node::StorageMember(v) => Some(v.path()),
        Node::Opaque(v) => Some(v.name()),
        Node::BitField(v) => Some(v.name()),
        Node::Variant(v) => Some(v.name()),
        Node::Scalar(_) | Node::Bits(_) | Node::Enum(_) | Node::Expression(_) => None,
    }
}
pub(super) const fn value_documentation(node: crate::ast::value::Node<'_>) -> Documentation {
    use crate::ast::value::Node;
    match node {
        Node::StorageField(v) => v.documentation(),
        Node::StorageMember(v) => v.documentation(),
        Node::Opaque(v) => v.documentation(),
        Node::BitField(v) => v.documentation(),
        Node::Variant(v) => v.documentation(),
        Node::Scalar(_) | Node::Bits(_) | Node::Enum(_) | Node::Expression(_) => {
            Documentation::EMPTY
        }
    }
}
