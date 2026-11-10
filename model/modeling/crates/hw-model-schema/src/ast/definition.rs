use crate::{Documentation, RecordSpec};
use std::borrow::Cow;
mod identity;
mod reference;
pub use identity::DefinitionId;
pub use reference::{Reference, ReferenceKind};

/// Structural metadata retained in addition to the definition's references.
#[derive(Debug, Clone)]
pub enum DefinitionBody {
    /// Scalar bit layout; byte order belongs to each containing field/register.
    Bits(super::BitsDefinition),
    /// A defined enumeration, distinct from its builtin integer representation.
    Enumeration(super::value::Enumeration),
    /// Whole-register metadata with field identifiers erased to declaration indices.
    Register(super::RegisterInfo),
    /// Parameterized placement rules, before choosing instance arguments.
    RegisterMap(super::MapDefinition),
    /// Ordinary fixed record, including scalar and bit-field metadata.
    Record(super::RecordDefinition),
    /// Union storage and optional selector path; alternatives are reference edges.
    Union(super::UnionDefinition),
    /// A custom layout declares geometry but no structural interpretation.
    Opaque(RecordSpec),
}

/// Original declaration form; rendering policy is chosen at each reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A reusable named declaration.
    Named,
    /// An anonymous declaration owned by a source occurrence.
    Inline,
}

/// One reusable definition, distinct from any placement or configured instance.
#[derive(Debug, Clone)]
pub struct Definition {
    id: DefinitionId,
    origin: Origin,
    source_member: Option<&'static str>,
    name: Cow<'static, str>,
    documentation: Documentation,
    body: DefinitionBody,
    references: Vec<Reference>,
}

impl Definition {
    /// Constructs an owned definition with independently selected identity and display name.
    pub const fn from_body(
        id: DefinitionId,
        name: String,
        documentation: Documentation,
        body: DefinitionBody,
    ) -> Self {
        Self {
            id,
            origin: Origin::Named,
            source_member: None,
            name: Cow::Owned(name),
            documentation,
            body,
            references: Vec::new(),
        }
    }
    pub(crate) fn rewritten(&self, body: DefinitionBody, references: Vec<Reference>) -> Self {
        Self {
            body,
            references,
            ..self.clone()
        }
    }
    pub(crate) fn extra_references(&self) -> &[Reference] {
        &self.references
    }
    /// Describes a reusable bit layout without imposing record encoding rules.
    pub fn bits<T: crate::BitsLayout + ?Sized + 'static>() -> Self {
        Self {
            id: DefinitionId::of::<T>(),
            origin: Origin::Named,
            source_member: None,
            name: Cow::Borrowed(T::NAME),
            documentation: T::DOCUMENTATION,
            body: DefinitionBody::Bits(super::BitsDefinition::of::<T>()),
            references: Vec::new(),
        }
    }
    /// Adds explicit references to a custom definition without using discovery.
    #[must_use]
    pub fn with_references(mut self, references: Vec<Reference>) -> Self {
        self.references = references;
        self
    }
    /// Describes a whole register while retaining all field semantics.
    pub fn register<T: ?Sized + 'static, I: Copy>(spec: crate::RegisterDefinition<I>) -> Self {
        Self {
            id: DefinitionId::of::<T>(),
            origin: Origin::Named,
            source_member: None,
            name: Cow::Borrowed(spec.name()),
            documentation: spec.documentation(),
            body: DefinitionBody::Register(super::RegisterInfo::from_spec(spec)),
            references: Vec::new(),
        }
    }
    /// Attaches a source bits reference to a register declaration.
    #[must_use]
    pub fn with_register_source(mut self, source: Reference) -> Self {
        if let DefinitionBody::Register(register) = self.body {
            self.body = DefinitionBody::Register(register.with_source(source));
        }
        self
    }
    /// Describes a map's placement rules and directly referenced definitions.
    pub fn register_map<T: ?Sized + 'static>(
        name: &'static str,
        documentation: Documentation,
        map: super::MapDefinition,
    ) -> Self {
        Self {
            id: DefinitionId::of::<T>(),
            origin: Origin::Named,
            source_member: None,
            name: Cow::Borrowed(name),
            documentation,
            body: DefinitionBody::RegisterMap(map),
            references: Vec::new(),
        }
    }
    /// Describes an ordinary record from logical source fields and storage layout.
    pub fn record<T: ?Sized + 'static>(spec: RecordSpec, fields: Vec<super::Field>) -> Self {
        Self::new::<T>(
            spec,
            DefinitionBody::Record(super::RecordDefinition::new(spec, fields)),
            Vec::new(),
        )
    }
    /// Attaches source alignment and byte order to a record declaration.
    #[must_use]
    pub fn with_record_declaration(
        mut self,
        alignment: Option<usize>,
        endian: crate::Endian,
    ) -> Self {
        if let DefinitionBody::Record(record) = self.body {
            self.body = DefinitionBody::Record(record.with_declaration(alignment, endian));
        }
        self
    }
    /// Describes a union without inventing a selection rule for untagged storage.
    pub fn union<T: ?Sized + 'static>(
        spec: RecordSpec,
        selector: Option<&'static str>,
        cases: Vec<Reference>,
    ) -> Self {
        Self::new::<T>(
            spec,
            DefinitionBody::Union(super::UnionDefinition::new(
                spec,
                selector,
                cases.into_iter().map(super::Case::external).collect(),
            )),
            Vec::new(),
        )
    }
    /// Describes a custom layout that need not expose its representation.
    pub fn opaque<T: ?Sized + 'static>(spec: RecordSpec) -> Self {
        Self::new::<T>(spec, DefinitionBody::Opaque(spec), Vec::new())
    }
    fn new<T: ?Sized + 'static>(
        spec: RecordSpec,
        body: DefinitionBody,
        references: Vec<Reference>,
    ) -> Self {
        Self {
            id: DefinitionId::of::<T>(),
            origin: Origin::Named,
            source_member: None,
            name: Cow::Borrowed(spec.name()),
            documentation: spec.documentation(),
            body,
            references,
        }
    }
    /// Declares source provenance without prescribing named or expanded rendering.
    #[must_use]
    pub const fn with_origin(mut self, origin: Origin) -> Self {
        self.origin = origin;
        self
    }
    /// Original DSL declaration form, preserved through graph construction.
    pub const fn origin(&self) -> Origin {
        self.origin
    }
    pub(crate) const fn at_member(mut self, source_member: &'static str) -> Self {
        self.source_member = Some(source_member);
        self
    }
    /// Original declaring member, when the definition was anonymous.
    pub const fn source_member(&self) -> Option<&'static str> {
        self.source_member
    }
    /// Returns identity independent of output naming.
    pub fn id(&self) -> DefinitionId {
        self.id.clone()
    }
    /// Returns the declared schema name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns definition documentation.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }
    /// Returns structural metadata without flattening reference relationships.
    pub const fn body(&self) -> &DefinitionBody {
        &self.body
    }
    /// Returns all references in declaration order.
    pub fn references(&self) -> Vec<&Reference> {
        self.node()
            .references()
            .chain(self.references.iter())
            .collect()
    }
    /// Borrows the canonical source declaration.
    pub const fn node(&self) -> super::Node<'_> {
        self.body.node()
    }
    /// Gives a reusable identity to an independently selected inline union.
    pub fn inline_union<T: ?Sized + 'static>(union: super::UnionDefinition) -> Self {
        Self::new::<T>(union.storage(), DefinitionBody::Union(union), Vec::new())
    }
}
