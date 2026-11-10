use super::Storage;
use crate::ast;

/// Borrowed record metadata without logical fields or storage members.
#[derive(Clone, Copy)]
pub struct Record<'a>(pub(crate) &'a ast::RecordDefinition);
impl Record<'_> {
    /// Physical storage geometry.
    pub const fn spec(self) -> Storage {
        Storage(self.0.spec())
    }
    /// Explicit source alignment.
    pub const fn declared_alignment(self) -> Option<usize> {
        self.0.declared_alignment()
    }
    /// Default byte order.
    pub const fn endian(self) -> crate::Endian {
        self.0.endian()
    }
}
/// Borrowed union geometry without alternatives.
#[derive(Clone, Copy)]
pub struct Union<'a>(pub(crate) &'a ast::UnionDefinition);
impl Union<'_> {
    /// Shared storage geometry.
    pub const fn storage(self) -> Storage {
        Storage(self.0.storage())
    }
    /// Selector supplied here; external selection can instead be bound on its reference.
    pub const fn selector(self) -> Option<&'static str> {
        self.0.selector()
    }
}
/// Reference edge metadata without a target factory or target definition.
#[derive(Clone, Copy)]
pub struct Reference<'a>(pub(crate) &'a ast::Reference);
impl Reference<'_> {
    /// Union selector interpreted in the reference's lexical owner scope.
    pub const fn selector_binding(self) -> Option<&'static str> {
        self.0.selector_binding()
    }

    /// Source edge name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Source relationship kind.
    pub const fn kind(self) -> ast::ReferenceKind {
        self.0.kind()
    }
    /// Optional union discriminant.
    pub const fn discriminant(self) -> Option<u64> {
        self.0.discriminant()
    }
}
/// Borrowed field placement without contained definitions.
#[derive(Clone, Copy)]
pub struct Field<'a>(pub(crate) &'a ast::Field);
impl Field<'_> {
    /// Declared field name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Placement documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
    /// Local byte offset.
    pub const fn offset(self) -> usize {
        self.0.offset()
    }
    /// Encoded byte extent.
    pub const fn width(self) -> usize {
        self.0.width()
    }
    /// Embedding byte order.
    pub const fn endian(self) -> crate::Endian {
        self.0.endian()
    }
}
/// Borrowed union alternative without its body.
#[derive(Clone, Copy)]
pub struct Case<'a>(pub(crate) &'a ast::Case);
impl Case<'_> {
    /// Declared alternative name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Optional wire selector value.
    pub const fn discriminant(self) -> Option<u64> {
        self.0.discriminant()
    }
}

impl core::fmt::Debug for Record<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Record")
            .field("name", &self.spec().name())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Union<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Union")
            .field("selector", &self.selector())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Reference<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Reference")
            .field("name", &self.name())
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Field<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Field")
            .field("name", &self.name())
            .field("offset", &self.offset())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Case<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Case")
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}
