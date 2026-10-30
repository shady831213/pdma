use super::{Artifact, Scope, emit, emit_all};
use crate::ast::{
    BitsDefinition, Case, Field, MapView, Node, PlacementView, RecordDefinition, RegisterInfo,
    UnionDefinition,
};
use crate::{BitsFieldSpec, EnumDefinition, EnumVariantSpec, RecordSpec};

/// Whole-node target syntax. Defaults descend through source containment only.
pub trait Backend: Sized {
    /// Error owned by the target implementation.
    type Error;
    /// Emits one complete definition or named view into target artifacts.
    fn definition(&mut self, node: Node<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit(node, scope, self)
    }
    /// Emits a complete source record.
    fn record(&mut self, value: &RecordDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::Record(value).children(), scope, self)
    }
    /// Emits a complete register.
    fn register(&mut self, value: &RegisterInfo, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::Register(value).children(), scope, self)
    }
    /// Emits a declared map or its checked permission projection.
    fn register_map(&mut self, value: MapView<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(value.children(), scope, self)
    }
    /// Emits a reusable or inline bits declaration.
    fn bits(&mut self, value: &BitsDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::Bits(value).children(), scope, self)
    }
    /// Emits an enumeration and its alternatives.
    fn enumeration(&mut self, value: EnumDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::Enum(value).children(), scope, self)
    }
    /// Emits a tagged or untagged union.
    fn union(&mut self, value: &UnionDefinition, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::Union(value).children(), scope, self)
    }
    /// Emits a logical record field.
    fn field(&mut self, value: &Field, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::Field(value).children(), scope, self)
    }
    /// Emits a bit field and any nested enum.
    fn bit_field(&mut self, value: &BitsFieldSpec, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::BitField(value).children(), scope, self)
    }
    /// Emits a register bit field, retaining its erased application index.
    fn register_field(
        &mut self,
        value: &BitsFieldSpec<usize>,
        scope: Scope<'_>,
    ) -> Result<(), Self::Error> {
        emit_all(Node::RegisterField(value).children(), scope, self)
    }
    /// Emits inline case fields or an external payload reference.
    fn case(&mut self, value: &Case, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(Node::Case(value).children(), scope, self)
    }
    /// Emits one enum alternative.
    fn variant(&mut self, _: &EnumVariantSpec, _: Scope<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Emits one source or concrete map placement.
    fn placement(&mut self, value: PlacementView<'_>, scope: Scope<'_>) -> Result<(), Self::Error> {
        emit_all(value.children(), scope, self)
    }
    /// Emits explicitly opaque storage metadata.
    fn opaque(&mut self, _: &RecordSpec, _: Scope<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Returns artifacts only after all selected definitions succeeded.
    fn finish(self) -> Result<Vec<Artifact>, Self::Error>;
}
