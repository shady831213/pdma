use super::Reference;
use crate::{Documentation, Endian, FieldDefinition, RecordSpec};

/// A declared constant constraint, including its original Rust symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Constant {
    /// Source expression naming the constant.
    pub symbol: &'static str,
    /// Evaluated unsigned wire value.
    pub value: u64,
}

/// Source type of one logical record field.
#[derive(Debug, Clone)]
pub enum FieldType {
    /// Integer scalar with an optional required value.
    Scalar {
        /// Scalar encoding attributes.
        definition: FieldDefinition,
        /// Declared constant constraint.
        constant: Option<Constant>,
    },
    /// Inline enumeration with scalar encoding attributes.
    Enum(FieldDefinition),
    /// Inline bit fields with scalar encoding attributes.
    Bits(super::BitsDefinition),
    /// Reusable record, union or bits definition.
    Reference(Reference),
    /// Inline union retaining its original selector and cases.
    Union(Box<UnionDefinition>),
}

/// One logical record field, before storage lowering.
#[derive(Debug, Clone)]
pub struct Field {
    name: &'static str,
    documentation: Documentation,
    offset: usize,
    extent: usize,
    endian: Endian,
    ty: FieldType,
}
impl Field {
    /// Constructs a source field with its placement attributes.
    pub const fn new(
        name: &'static str,
        offset: usize,
        extent: usize,
        endian: Endian,
        ty: FieldType,
    ) -> Self {
        Self {
            name,
            documentation: Documentation::EMPTY,
            offset,
            extent,
            endian,
            ty,
        }
    }
    /// Attaches source field documentation.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: Documentation) -> Self {
        self.documentation = documentation;
        self
    }
    /// Declared field name.
    pub const fn name(&self) -> &'static str {
        self.name
    }
    /// Source documentation.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }
    /// Byte offset within the record.
    pub const fn offset(&self) -> usize {
        self.offset
    }
    /// Encoded byte extent.
    pub const fn width(&self) -> usize {
        self.extent
    }
    /// Byte order at this embedding.
    pub const fn endian(&self) -> Endian {
        self.endian
    }
    /// Source field type without storage flattening.
    pub const fn ty(&self) -> &FieldType {
        &self.ty
    }
}

/// Source record declaration and its physical layout attributes.
#[derive(Debug, Clone)]
pub struct RecordDefinition {
    spec: RecordSpec,
    fields: Vec<Field>,
    declared_alignment: Option<usize>,
    endian: Endian,
}
impl RecordDefinition {
    /// Creates a record from source fields, independently of storage metadata.
    pub const fn new(spec: RecordSpec, fields: Vec<Field>) -> Self {
        Self {
            spec,
            fields,
            declared_alignment: None,
            endian: Endian::Little,
        }
    }
    /// Records the source alignment clause and default byte order.
    #[must_use]
    pub const fn with_declaration(mut self, alignment: Option<usize>, endian: Endian) -> Self {
        self.declared_alignment = alignment;
        self.endian = endian;
        self
    }
    /// Explicit source alignment, absent when inferred.
    pub const fn declared_alignment(&self) -> Option<usize> {
        self.declared_alignment
    }
    /// Declared default byte order for fields in this record.
    pub const fn endian(&self) -> Endian {
        self.endian
    }
    /// Physical encoding layout, which may lower inline unions to storage words.
    pub const fn spec(&self) -> RecordSpec {
        self.spec
    }
    /// Logical fields in source declaration order.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}

/// Source union declaration; cases retain independent definition references.
#[derive(Debug, Clone)]
pub struct UnionDefinition {
    storage: RecordSpec,
    selector: Option<&'static str>,
    cases: Vec<Case>,
}
impl UnionDefinition {
    /// Creates a tagged or untagged union from source alternatives.
    pub const fn new(
        storage: RecordSpec,
        selector: Option<&'static str>,
        cases: Vec<Case>,
    ) -> Self {
        Self {
            storage,
            selector,
            cases,
        }
    }
    /// Shared storage geometry.
    pub const fn storage(&self) -> RecordSpec {
        self.storage
    }
    /// Original selector path, absent for an untagged union.
    pub const fn selector(&self) -> Option<&'static str> {
        self.selector
    }
    /// Alternatives in declaration order.
    pub fn cases(&self) -> &[Case] {
        &self.cases
    }
}

/// Original payload declaration of a union alternative.
#[derive(Debug, Clone)]
pub enum CaseBody {
    /// Fields declared directly inside this alternative.
    Inline(RecordDefinition),
    /// An external codec type named by the alternative.
    Reference(Reference),
}

/// One source union alternative, distinct from its optional reusable payload.
#[derive(Debug, Clone)]
pub struct Case {
    name: &'static str,
    discriminant: Option<u64>,
    body: CaseBody,
}
impl Case {
    /// Constructs an inline case whose fields remain owned containment.
    pub const fn inline(
        name: &'static str,
        discriminant: Option<u64>,
        record: RecordDefinition,
    ) -> Self {
        Self {
            name,
            discriminant,
            body: CaseBody::Inline(record),
        }
    }
    /// Constructs a case referring to an independently reusable codec.
    pub const fn external(reference: Reference) -> Self {
        Self {
            name: reference.name(),
            discriminant: reference.discriminant(),
            body: CaseBody::Reference(reference),
        }
    }
    /// Declared case name.
    pub const fn name(&self) -> &'static str {
        self.name
    }
    /// Declared wire selector value, absent in an untagged union.
    pub const fn discriminant(&self) -> Option<u64> {
        self.discriminant
    }
    /// Original inline fields or external codec reference.
    pub const fn body(&self) -> &CaseBody {
        &self.body
    }
}
