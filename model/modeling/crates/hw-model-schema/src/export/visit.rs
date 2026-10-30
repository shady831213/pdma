//! Formatting callbacks driven by the common schema walker.
mod nodes;
use super::{Artifact, BankView, PreparedMap, PreparedReference, RegisterMapView};
use crate::{
    Documentation, FieldDefinition, RecordSpec, ResolvedMember, ResolvedRecord,
    graph::{BitsDefinition, MapLayoutPlan},
};
pub use nodes::{
    BitField, ChildCall, DefinitionHeader, EnumSite, ExpressionReference, FieldOwner,
    GeometryAssignment, GeometryMember, GeometrySignature, PlacementProperty, RegisterLocation,
    RegisterNode,
};

/// Target formatting actions. Traversal, recursion and layout evaluation are common.
pub trait Renderer: Sized {
    /// Target-specific failure.
    type Error;
    /// Opens a definition artifact or section.
    fn begin_definition(&mut self, _: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one dependency edge.
    fn reference(&mut self, _: &PreparedReference) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes a definition artifact or section.
    fn end_definition(&mut self, _: DefinitionHeader<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens record formatting; target name preflight may inspect member names.
    fn begin_record(&mut self, _: &ResolvedRecord) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Visits each resolved member in declaration order, including nested containers.
    fn record_member(&mut self, _: usize, _: &ResolvedMember) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders nested-record documentation.
    fn record_group(&mut self, _: &ResolvedMember, _: &RecordSpec) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens one scalar member before its bit/enum children.
    fn begin_record_field(
        &mut self,
        _: usize,
        _: &ResolvedMember,
        _: FieldDefinition,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one scalar's layout/accessors after its bit/enum children.
    fn end_record_field(
        &mut self,
        _: usize,
        _: &ResolvedMember,
        _: FieldDefinition,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one nested record's placement metadata.
    fn record_placement(&mut self, _: &ResolvedMember, _: &RecordSpec) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes a record.
    fn end_record(&mut self, _: &ResolvedRecord) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens a standalone bit layout.
    fn begin_bits(&mut self, _: &BitsDefinition) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one bit field.
    fn bit_field(&mut self, _: BitField<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one enum discriminant and its documentation.
    fn enum_variant(
        &mut self,
        _: EnumSite<'_>,
        _: crate::EnumVariantSpec,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes a standalone bit layout.
    fn end_bits(&mut self, _: &BitsDefinition) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens a map; this callback must not walk children.
    fn begin_map(&mut self, _: &PreparedMap) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens concrete-map formatting; target name preflight may inspect member names.
    fn begin_map_instance(&mut self, _: &RegisterMapView) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one nested group's documentation.
    fn map_group(&mut self, _: &str, _: Documentation) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens one bank declaration.
    fn begin_bank(&mut self, _: usize, _: &BankView, _: bool) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes one bank declaration.
    fn end_bank(&mut self, _: usize, _: &BankView, _: bool) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens one register before its bit/enum children.
    fn begin_register(&mut self, _: RegisterNode<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes one register after its bit/enum children.
    fn end_register(&mut self, _: RegisterNode<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes a concrete map.
    fn end_map_instance(&mut self, _: &RegisterMapView) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes a map.
    fn end_map(&mut self, _: &PreparedMap) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Opens a geometry result type; no placement iteration belongs here.
    fn begin_geometry(&mut self, _: &MapLayoutPlan) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one result member.
    fn geometry_member(&mut self, _: GeometryMember<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes the result type.
    fn end_geometry_members(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Formats one parameter declaration.
    fn geometry_parameter(&mut self, name: &str) -> Result<String, Self::Error> {
        Ok(name.into())
    }
    /// Opens the layout function after common parameter traversal.
    fn begin_geometry_body(
        &mut self,
        _: &MapLayoutPlan,
        _: &GeometrySignature,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one child call with already rendered arguments.
    fn geometry_child_call(&mut self, _: ChildCall<'_>, _: &[String]) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders one assignment with an already rendered expression.
    fn geometry_assignment(
        &mut self,
        _: GeometryAssignment<'_>,
        _: &str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes the layout function.
    fn end_geometry_body(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Renders an optional property accessor; traversal is shared.
    fn geometry_accessor(
        &mut self,
        _: &str,
        _: PlacementProperty,
        _: &GeometrySignature,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Closes geometry formatting.
    fn end_geometry(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Formats a nonnegative geometry literal.
    fn expression_constant(&mut self, value: usize) -> String {
        value.to_string()
    }
    /// Escapes an expression-local source identifier.
    fn expression_identifier(&mut self, name: &str) -> String {
        name.into()
    }
    /// Formats a resolved reference; the default uses C-like member syntax.
    fn expression_reference(&mut self, reference: ExpressionReference<'_>) -> String {
        super::walk::expression::reference(self, reference)
    }
    /// Formats folded operands without walking their expressions again.
    fn expression_binary(
        &mut self,
        op: crate::graph::ExpressionOp,
        left: &str,
        right: &str,
    ) -> String {
        super::walk::expression::binary(self, op, left, right)
    }
    /// Formats the target's maximum expression from rendered operands.
    fn expression_maximum(&mut self, left: &str, right: &str) -> String {
        format!("({left} > {right} ? {left} : {right})")
    }
    /// Completes all artifacts after traversal succeeds.
    fn finish(self) -> Result<Vec<Artifact>, Self::Error>;
}
