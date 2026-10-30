use super::super::{
    ChildCall, ExportError, ExpressionReference, GeometryAssignment, GeometryMember,
    GeometrySignature, PlacementProperty, Renderer,
};
use super::expression;
use crate::graph::{ChildLayout, MapLayoutPlan};

pub(super) fn walk<R: Renderer, S>(
    r: &mut R,
    plan: &MapLayoutPlan,
) -> Result<(), ExportError<R::Error, S>> {
    r.begin_geometry(plan).map_err(ExportError::Render)?;
    r.geometry_member(GeometryMember::Extent)
        .map_err(ExportError::Render)?;
    for placement in plan.placements() {
        for property in [
            PlacementProperty::Offset,
            PlacementProperty::Count,
            PlacementProperty::Stride,
        ] {
            r.geometry_member(GeometryMember::Placement {
                name: placement.reference.name(),
                property,
            })
            .map_err(ExportError::Render)?;
        }
    }
    r.end_geometry_members().map_err(ExportError::Render)?;
    let mut signature = GeometrySignature::default();
    for parameter in plan.parameters() {
        signature.parameters.push(
            r.geometry_parameter(parameter)
                .map_err(ExportError::Render)?,
        );
        signature
            .arguments
            .push(r.expression_reference(ExpressionReference::Parameter(parameter)));
    }
    r.begin_geometry_body(plan, &signature)
        .map_err(ExportError::Render)?;
    for placement in plan.placements() {
        let name = placement.reference.name();
        if let ChildLayout::Call { arguments, .. } = &placement.child {
            let arguments = arguments
                .iter()
                .map(|arg| expression::render(r, arg, None))
                .collect::<Result<Vec<_>, _>>()
                .map_err(ExportError::Layout)?;
            r.geometry_child_call(
                ChildCall {
                    name,
                    definition: placement.child_definition,
                    definition_name: placement.child_name,
                },
                &arguments,
            )
            .map_err(ExportError::Render)?;
        }
        for (property, expr) in [
            (PlacementProperty::Offset, &placement.offset),
            (PlacementProperty::Count, &placement.count),
            (PlacementProperty::Stride, &placement.stride),
        ] {
            let value =
                expression::render(r, expr, Some(placement)).map_err(ExportError::Layout)?;
            r.geometry_assignment(
                GeometryAssignment::Member(GeometryMember::Placement { name, property }),
                &value,
            )
            .map_err(ExportError::Render)?;
        }
        if !plan.parameters().is_empty() {
            let end = expression::render(r, &placement.end, Some(placement))
                .map_err(ExportError::Layout)?;
            r.geometry_assignment(GeometryAssignment::End(name), &end)
                .map_err(ExportError::Render)?;
            let cursor = expression::render(r, &placement.cursor, Some(placement))
                .map_err(ExportError::Layout)?;
            r.geometry_assignment(GeometryAssignment::Cursor, &cursor)
                .map_err(ExportError::Render)?;
        }
    }
    let extent = expression::render(r, plan.extent(), None).map_err(ExportError::Layout)?;
    r.geometry_assignment(GeometryAssignment::Member(GeometryMember::Extent), &extent)
        .map_err(ExportError::Render)?;
    r.end_geometry_body().map_err(ExportError::Render)?;
    for placement in plan.placements() {
        for property in [
            PlacementProperty::Offset,
            PlacementProperty::Count,
            PlacementProperty::Stride,
        ] {
            r.geometry_accessor(placement.reference.name(), property, &signature)
                .map_err(ExportError::Render)?;
        }
    }
    r.end_geometry().map_err(ExportError::Render)
}
