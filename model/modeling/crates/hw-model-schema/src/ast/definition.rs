use std::any::{TypeId, type_name};

use crate::{Documentation, RecordSpec};

/// Process-local identity plus a deterministic diagnostic/display path.
///
/// Type identity distinguishes versions of the same crate. The Rust path alone
/// is not identity; consumers must diagnose collisions in output names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefinitionId {
    identity: TypeId,
    path: &'static str,
}

impl DefinitionId {
    /// Identifies a Rust definition; reexports retain the same identity.
    pub fn of<T: ?Sized + 'static>() -> Self {
        Self {
            identity: TypeId::of::<T>(),
            path: type_name::<T>(),
        }
    }
    /// Returns the defining Rust type path, not a generated target symbol.
    pub const fn path(self) -> &'static str {
        self.path
    }
}

/// Why one schema definition refers to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A record member uses another definition.
    Field,
    /// A union alternative uses another definition.
    Variant,
    /// A register map places a child definition.
    Placement,
}

/// One reference edge; duplicate targets do not discard distinct field edges.
#[derive(Debug, Clone)]
pub struct Reference {
    name: &'static str,
    kind: ReferenceKind,
    target: fn() -> Definition,
    discriminant: Option<u64>,
}

impl Reference {
    /// Creates an edge whose target can be obtained without a global registry.
    pub const fn new(name: &'static str, kind: ReferenceKind, target: fn() -> Definition) -> Self {
        Self {
            name,
            kind,
            target,
            discriminant: None,
        }
    }
    /// Attaches a tagged-union wire discriminant.
    #[must_use]
    pub const fn with_discriminant(mut self, value: u64) -> Self {
        self.discriminant = Some(value);
        self
    }
    /// Field, alternative or placement name in its owner.
    pub const fn name(&self) -> &'static str {
        self.name
    }
    /// Returns the relationship kind.
    pub const fn kind(&self) -> ReferenceKind {
        self.kind
    }
    /// Returns the selected alternative's wire value, when tagged.
    pub const fn discriminant(&self) -> Option<u64> {
        self.discriminant
    }
    /// Obtains the referenced definition, without following its own edges.
    pub fn target(&self) -> Definition {
        (self.target)()
    }
}

/// Structural metadata retained in addition to the definition's references.
#[derive(Debug, Clone)]
pub enum DefinitionBody {
    /// Scalar bit layout; byte order belongs to each containing field/register.
    Bits(super::BitsDefinition),
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

/// One reusable definition, distinct from any placement or configured instance.
#[derive(Debug, Clone)]
pub struct Definition {
    id: DefinitionId,
    name: &'static str,
    documentation: Documentation,
    body: DefinitionBody,
    references: Vec<Reference>,
}

impl Definition {
    /// Describes a reusable bit layout without imposing record encoding rules.
    pub fn bits<T: crate::BitsLayout + ?Sized + 'static>() -> Self {
        Self {
            id: DefinitionId::of::<T>(),
            name: T::NAME,
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
            name: spec.name(),
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
            name,
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
            name: spec.name(),
            documentation: spec.documentation(),
            body,
            references,
        }
    }
    /// Returns identity independent of output naming.
    pub const fn id(&self) -> DefinitionId {
        self.id
    }
    /// Returns the declared schema name.
    pub const fn name(&self) -> &'static str {
        self.name
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
