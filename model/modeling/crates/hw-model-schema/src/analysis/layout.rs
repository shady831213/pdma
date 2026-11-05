//! Derives complete semantic geometry for each resolved definition.
use super::{AnalysisError, Bindings, Dependency, Layout, MapLayout, RegisterMapView, Unit};
use crate::ast::{DefinitionBody, DefinitionId, SchemaGraph};
pub(super) fn run(graph: &SchemaGraph, bindings: &Bindings) -> Result<Vec<Unit>, AnalysisError> {
    let mut units: Vec<Unit> = graph
        .dependency_order()
        .map(|definition| prepare_definition(definition, bindings))
        .collect::<Result<_, AnalysisError>>()?;
    for instance in graph.instances() {
        let definition = graph
            .get(instance.definition())
            .ok_or_else(|| AnalysisError::Binding(instance.definition()))?;
        let DefinitionBody::RegisterMap(map) = definition.body() else {
            return Err(AnalysisError::Binding(instance.definition()));
        };
        units.push(Unit {
            declaration: Some(definition.body().clone()),
            id: crate::traversal::NodeId::view(instance.name()),
            name: instance.name().into(),
            documentation: definition.documentation(),
            references: vec![Dependency {
                name: "definition",
                kind: crate::ast::ReferenceKind::Placement,
                target: crate::traversal::NodeId::definition(definition.id()),
                discriminant: None,
            }],
            body: Layout::Map(MapLayout {
                plan: Some(Box::new(super::CompleteMapLayout::resolve(
                    &map.layout_plan()?,
                    Some(instance.arguments()),
                )?)),
                arguments: Some(instance.arguments().to_vec()),
                instance: Some(RegisterMapView::from_definition(
                    instance.name(),
                    definition.documentation(),
                    map,
                    instance.arguments(),
                )?),
            }),
        });
    }
    Ok(units)
}

fn prepare_definition(
    definition: &crate::ast::Definition,
    bindings: &Bindings,
) -> Result<Unit, AnalysisError> {
    let body = match definition.body() {
        DefinitionBody::Bits(_) | DefinitionBody::Register(_) => Layout::None,
        DefinitionBody::Record(record) => resolve_record(record.spec(), definition.id())?,
        DefinitionBody::Opaque(spec) => resolve_record(*spec, definition.id())?,
        DefinitionBody::Union(union) => resolve_record(union.storage(), definition.id())?,
        DefinitionBody::RegisterMap(map) => {
            let plan = map.layout_plan()?;
            let args = bindings
                .values
                .get(&definition.id())
                .map(Vec::as_slice)
                .or_else(|| map.parameters().is_empty().then_some(&[]));
            let instance = args
                .map(|args| {
                    RegisterMapView::from_definition(
                        definition.name(),
                        definition.documentation(),
                        map,
                        args,
                    )
                })
                .transpose()?;
            let plan = super::CompleteMapLayout::resolve(&plan, args)?;
            Layout::Map(MapLayout {
                plan: Some(Box::new(plan)),
                arguments: args.map(<[usize]>::to_vec),
                instance,
            })
        }
    };
    Ok(Unit {
        declaration: Some(definition.body().clone()),
        id: crate::traversal::NodeId::definition(definition.id()),
        name: definition.name().into(),
        documentation: definition.documentation(),
        body,
        references: definition
            .references()
            .into_iter()
            .map(Dependency::from)
            .collect(),
    })
}

fn resolve_record(spec: crate::RecordSpec, id: DefinitionId) -> Result<Layout, AnalysisError> {
    spec.resolve()
        .map(Layout::Record)
        .map_err(|source| AnalysisError::Record {
            path: id.path(),
            source,
        })
}
