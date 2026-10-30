use super::{
    BitsDefinition, Case, CaseBody, DefinitionBody, Field, FieldType, MapView, PlacementView,
    RecordDefinition, Reference, RegisterInfo, UnionDefinition,
};
use crate::{BitsFieldSpec, EnumDefinition, EnumVariantSpec, RecordSpec};

/// A borrowed view of an original schema declaration or one of its members.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// Record declaration.
    Record(&'a RecordDefinition),
    /// Register declaration.
    Register(&'a RegisterInfo),
    /// Register-map declaration.
    RegisterMap(MapView<'a>),
    /// Reusable bits declaration.
    Bits(&'a BitsDefinition),
    /// Inline enumeration; its variant slice is borrowed from the declaration.
    Enum(EnumDefinition),
    /// Union declaration.
    Union(&'a UnionDefinition),
    /// Logical record field.
    Field(&'a Field),
    /// Bit field of a reusable or embedded bits declaration.
    BitField(&'a BitsFieldSpec),
    /// Register bit field with erased application identity.
    RegisterField(&'a BitsFieldSpec<usize>),
    /// Union alternative containing fields or referencing an external payload.
    Case(&'a Case),
    /// Enumeration alternative.
    Variant(&'a EnumVariantSpec),
    /// Register-map placement.
    Placement(PlacementView<'a>),
    /// Manually described opaque storage.
    Opaque(&'a RecordSpec),
}
impl<'a> Node<'a> {
    /// Iterates containment in source order, without following definition references.
    pub fn children(self) -> std::vec::IntoIter<Self> {
        let nodes = match self {
            Self::Record(record) => record.fields().iter().map(Self::Field).collect(),
            Self::Register(register) => {
                if register.source().is_some() {
                    Vec::new()
                } else {
                    register.fields().iter().map(Self::RegisterField).collect()
                }
            }
            Self::RegisterMap(map) => map.children().collect(),
            Self::Bits(bits) => bits.fields().iter().map(Self::BitField).collect(),
            Self::Enum(enumeration) => enumeration.variants().iter().map(Self::Variant).collect(),
            Self::Union(union) => union.cases().iter().map(Self::Case).collect(),
            Self::Field(field) => match field.ty() {
                FieldType::Enum(definition) => definition
                    .enum_definition()
                    .map(Self::Enum)
                    .into_iter()
                    .collect(),
                FieldType::Bits(definition) => vec![Self::Bits(definition)],
                FieldType::Union(union) => vec![Self::Union(union)],
                FieldType::Reference(_) | FieldType::Scalar { .. } => Vec::new(),
            },
            Self::BitField(field) => field
                .definition()
                .enum_definition()
                .map(Self::Enum)
                .into_iter()
                .collect(),
            Self::RegisterField(field) => field
                .definition()
                .enum_definition()
                .map(Self::Enum)
                .into_iter()
                .collect(),
            Self::Placement(placement) => placement.children().collect(),
            Self::Case(case) => match case.body() {
                CaseBody::Inline(record) => record.fields().iter().map(Self::Field).collect(),
                CaseBody::Reference(_) => Vec::new(),
            },
            Self::Variant(_) | Self::Opaque(_) => Vec::new(),
        };
        nodes.into_iter()
    }
    /// Iterates dependency edges throughout this containment subtree, retaining duplicates.
    pub fn references(self) -> std::vec::IntoIter<&'a Reference> {
        let mut references = Vec::new();
        self.collect_references(&mut references);
        references.into_iter()
    }
    fn collect_references(self, references: &mut Vec<&'a Reference>) {
        match self {
            Self::Field(field) => {
                if let FieldType::Reference(reference) = field.ty() {
                    references.push(reference);
                }
            }
            Self::Register(register) => references.extend(register.source()),
            Self::Case(case) => match case.body() {
                CaseBody::Reference(reference) => references.push(reference),
                CaseBody::Inline(_) => {}
            },
            Self::Placement(PlacementView::Declared(placement)) => {
                references.push(placement.reference());
            }
            Self::Placement(PlacementView::Register { .. } | PlacementView::Bank { .. })
            | Self::Record(_)
            | Self::RegisterMap(_)
            | Self::Bits(_)
            | Self::Enum(_)
            | Self::Union(_)
            | Self::BitField(_)
            | Self::RegisterField(_)
            | Self::Variant(_)
            | Self::Opaque(_) => {}
        }
        for child in self.children() {
            child.collect_references(references);
        }
    }
}
impl DefinitionBody {
    /// Borrows the original declaration without constructing another representation.
    pub const fn node(&self) -> Node<'_> {
        match self {
            Self::Record(value) => Node::Record(value),
            Self::Register(value) => Node::Register(value),
            Self::RegisterMap(value) => Node::RegisterMap(MapView::Declared(value)),
            Self::Bits(value) => Node::Bits(value),
            Self::Union(value) => Node::Union(value),
            Self::Opaque(value) => Node::Opaque(value),
        }
    }
}
