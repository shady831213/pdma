use crate::analysis::{CompleteMapLayout, CompletePlacementLayout};
use crate::ast::{
    DefinitionBody, MapDefinition, MapPlacement, PlacementView,
    register::{Argument, Attribute, LayoutProperty, LayoutPropertyKind, Node, Parameter},
};
pub(super) fn map<'a>(
    map: &'a MapDefinition,
    plan: Option<&'a CompleteMapLayout>,
    arguments: Option<&[usize]>,
) -> Vec<Node<'a>> {
    let mut nodes: Vec<_> = map
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, name)| {
            Node::Parameter(Parameter {
                name,
                index,
                binding: arguments.and_then(|a| a.get(index).copied()),
            })
        })
        .collect();
    nodes.push(Node::Attribute(Attribute::Alignment(map.alignment())));
    nodes.extend(
        map.explicit_extent()
            .map(|value| Node::Attribute(Attribute::ExplicitExtent(value))),
    );
    nodes.extend(
        map.placements()
            .iter()
            .enumerate()
            .map(|(index, declaration)| {
                Node::Placement(
                    plan.and_then(|p| p.placements().get(index))
                        .map_or(PlacementView::Declared(declaration), |plan| {
                            PlacementView::Analyzed { declaration, plan }
                        }),
                )
            }),
    );
    if let Some(plan) = plan {
        nodes.push(Node::LayoutProperty(LayoutProperty {
            kind: LayoutPropertyKind::Extent,
            expression: plan.extent(),
        }));
    }
    nodes
}
pub(super) fn placement<'a>(
    placement: &'a MapPlacement,
    plan: Option<&'a CompletePlacementLayout>,
) -> Vec<Node<'a>> {
    let parameters = match placement.reference().target().body() {
        DefinitionBody::RegisterMap(map) => map.parameters(),
        DefinitionBody::Bits(_)
        | DefinitionBody::Register(_)
        | DefinitionBody::Record(_)
        | DefinitionBody::Union(_)
        | DefinitionBody::Opaque(_) => &[],
    };
    let mut nodes: Vec<_> = placement
        .arguments()
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            Node::Argument(Argument {
                parameter: parameters.get(index).copied(),
                index,
                expression,
            })
        })
        .collect();
    nodes.push(Node::Attribute(Attribute::Count(placement.count())));
    nodes.push(Node::Attribute(Attribute::Bank(placement.is_bank())));
    nodes.extend(
        placement
            .offset()
            .map(|value| Node::Attribute(Attribute::ExplicitOffset(value))),
    );
    if let Some(plan) = plan {
        nodes.extend(
            [
                (LayoutPropertyKind::Offset, &plan.offset),
                (LayoutPropertyKind::Count, &plan.count),
                (LayoutPropertyKind::Stride, &plan.stride),
                (LayoutPropertyKind::End, &plan.end),
                (LayoutPropertyKind::OccupiedEnd, &plan.cursor),
            ]
            .map(|(kind, expression)| Node::LayoutProperty(LayoutProperty { kind, expression })),
        );
    }
    nodes
}
