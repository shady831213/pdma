use super::super::{ExportId, RegisterView, SchemaKind};
use crate::{
    Documentation, ResolvedMember,
    graph::{DefinitionId, RegisterInfo},
};

/// Definition identity and documentation, without a traversable body.
#[derive(Debug, Clone, Copy)]
pub struct DefinitionHeader<'a> {
    /// Stable definition or view identity.
    pub id: &'a ExportId,
    /// Declared name.
    pub name: &'a str,
    /// Original schema kind.
    pub kind: SchemaKind,
    /// Root documentation.
    pub documentation: Documentation,
}
/// Register declaration position in the common traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterLocation {
    /// Standalone register definition.
    Root,
    /// A concrete map's standalone or expanded register.
    Standalone(usize),
    /// Bank template member.
    Template {
        /// Bank declaration index.
        bank: usize,
        /// Template member index.
        member: usize,
    },
    /// Bank instance member.
    Instance {
        /// Bank declaration index.
        bank: usize,
        /// Selected bank instance.
        instance: usize,
        /// Member within that instance.
        member: usize,
    },
}
/// One register visit, retaining effective metadata and its source placement.
#[derive(Debug, Clone, Copy)]
pub struct RegisterNode<'a> {
    /// Structural position, independent of target naming.
    pub location: RegisterLocation,
    /// Register semantics.
    pub register: &'a RegisterInfo,
    /// Placement metadata; absent for a standalone register definition.
    pub view: Option<&'a RegisterView>,
}
/// Semantic owner of bit fields and enum variants.
#[derive(Debug, Clone, Copy)]
pub enum FieldOwner<'a> {
    /// A standalone bits declaration.
    Bits,
    /// A scalar member of a record.
    Record {
        /// Scalar declaration index in the resolved traversal.
        index: usize,
        /// Scalar placement metadata.
        member: &'a ResolvedMember,
    },
    /// A register declaration or instance.
    Register(RegisterNode<'a>),
}
/// One bit-field visit; enum children are visited separately by the walker.
#[derive(Debug, Clone, Copy)]
pub struct BitField<'a> {
    /// Field owner.
    pub owner: FieldOwner<'a>,
    /// Declared name.
    pub name: &'static str,
    /// Least-significant bit.
    pub lsb: usize,
    /// Encoded bit width.
    pub width: usize,
    /// Effective documentation.
    pub documentation: Documentation,
}
/// Location of a scalar or bit-field enum.
#[derive(Debug, Clone, Copy)]
pub struct EnumSite<'a> {
    /// Enclosing scalar or register.
    pub owner: FieldOwner<'a>,
    /// Bit field name, absent for a scalar enum.
    pub bit: Option<&'static str>,
}
/// Public geometry fields shared by targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementProperty {
    /// First child byte offset.
    Offset,
    /// Child instance count.
    Count,
    /// Child instance spacing.
    Stride,
}
impl PlacementProperty {
    /// Canonical property name; target escaping remains renderer-owned.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Offset => "offset",
            Self::Count => "count",
            Self::Stride => "stride",
        }
    }
}
/// A geometry result member.
#[derive(Debug, Clone, Copy)]
pub enum GeometryMember<'a> {
    /// Overall occupied extent.
    Extent,
    /// One property of a placed child.
    Placement {
        /// Source placement name.
        name: &'a str,
        /// Selected property.
        property: PlacementProperty,
    },
}
/// A resolved symbolic reference, requiring no backend layout interpretation.
#[derive(Debug, Clone, Copy)]
pub enum ExpressionReference<'a> {
    /// Function parameter.
    Parameter(&'a str),
    /// Running occupied extent.
    Cursor,
    /// Previously assigned placement property.
    Placement {
        /// Source placement name.
        name: &'a str,
        /// Selected property.
        property: PlacementProperty,
    },
    /// Extent returned by a child layout call.
    ChildExtent(&'a str),
    /// Local exclusive end of a placement.
    End(&'a str),
}
/// One layout assignment; the expression has already been rendered recursively.
#[derive(Debug, Clone, Copy)]
pub enum GeometryAssignment<'a> {
    /// Assignment to a returned geometry field.
    Member(GeometryMember<'a>),
    /// Declaration of a local exclusive end.
    End(&'a str),
    /// Update to the running occupied extent.
    Cursor,
}
/// A validated call to a child layout.
#[derive(Debug, Clone, Copy)]
pub struct ChildCall<'a> {
    /// Local placement name.
    pub name: &'a str,
    /// Referenced definition identity.
    pub definition: DefinitionId,
    /// Referenced definition's default name.
    pub definition_name: &'static str,
}
/// Formatted parameter declarations and corresponding call arguments.
#[derive(Debug, Default)]
pub struct GeometrySignature {
    /// One formatted declaration per source parameter.
    pub parameters: Vec<String>,
    /// One formatted reference per source parameter.
    pub arguments: Vec<String>,
}
