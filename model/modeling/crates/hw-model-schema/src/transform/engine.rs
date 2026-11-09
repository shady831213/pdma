use super::{Occurrence, TransformError};
use crate::{
    Documentation,
    ast::{
        Definition, DefinitionBody, DefinitionId, Reference, ReferenceKind, SchemaGraph, SchemaSet,
    },
};
use std::collections::HashMap;
pub(super) struct Engine<'a, S, N> {
    graph: &'a SchemaGraph,
    pub(super) outline: bool,
    select: S,
    name: N,
    complete: HashMap<DefinitionId, Definition>,
    names: HashMap<String, DefinitionId>,
}
impl<'a, S, N> Engine<'a, S, N>
where
    S: FnMut(&Occurrence) -> bool,
    N: FnMut(&Occurrence) -> String,
{
    pub(super) fn new(graph: &'a SchemaGraph, outline: bool, select: S, name: N) -> Self {
        Self {
            graph,
            outline,
            select,
            name,
            complete: HashMap::new(),
            names: graph
                .definitions()
                .map(|d| (d.name().to_owned(), d.id()))
                .collect(),
        }
    }
    pub(super) fn run(mut self) -> Result<SchemaSet, TransformError> {
        for definition in self.graph.dependency_order() {
            let site = Occurrence {
                owner: definition.id(),
                owner_name: definition.name().to_owned(),
                path: Vec::new(),
                identity: Vec::new(),
            };
            let body = self.body(definition.body(), &site)?;
            let references = definition
                .extra_references()
                .iter()
                .enumerate()
                .map(|(index, reference)| {
                    let site = site.child("edge", index, reference.name());
                    if !self.outline && self.selected(&site) {
                        return Err(TransformError::Unsupported { path: site.path });
                    }
                    self.reference(reference)
                })
                .collect::<Result<_, _>>()?;
            self.complete
                .insert(definition.id(), definition.rewritten(body, references));
        }
        let mut set = SchemaSet::new().with_instances(self.graph.instances());
        for id in self.graph.roots() {
            set.add_definition(
                self.complete
                    .get(id)
                    .ok_or_else(|| TransformError::Missing(id.clone()))?
                    .clone(),
            );
        }
        Ok(set)
    }
    pub(super) fn selected(&mut self, site: &Occurrence) -> bool {
        (self.select)(site)
    }
    pub(super) fn reference(&self, reference: &Reference) -> Result<Reference, TransformError> {
        let id = reference.target().id();
        self.complete
            .get(&id)
            .cloned()
            .map(|d| reference.retarget(d))
            .ok_or(TransformError::Missing(id))
    }
    pub(super) fn target(
        reference: &Reference,
        site: &Occurrence,
    ) -> Result<Definition, TransformError> {
        let target = reference.target();
        if !target.extra_references().is_empty() {
            return Err(TransformError::Unsupported {
                path: site.path.clone(),
            });
        }
        if let Some(selector) = reference.selector_binding() {
            let DefinitionBody::Union(union) = target.body() else {
                return Err(TransformError::Unsupported {
                    path: site.path.clone(),
                });
            };
            return Ok(target.rewritten(
                DefinitionBody::Union(crate::ast::UnionDefinition::new(
                    union.storage(),
                    Some(selector),
                    union.cases().to_vec(),
                )),
                Vec::new(),
            ));
        }
        Ok(target)
    }
    pub(super) fn lift(
        &mut self,
        body: DefinitionBody,
        site: &Occurrence,
        member: &'static str,
        kind: ReferenceKind,
        fallback: Documentation,
    ) -> Result<Reference, TransformError> {
        let (body, selector) = match body {
            DefinitionBody::Union(union) => {
                let selector = union.selector();
                (
                    DefinitionBody::Union(crate::ast::UnionDefinition::new(
                        union.storage(),
                        None,
                        union.cases().to_vec(),
                    )),
                    selector,
                )
            }
            other @ (DefinitionBody::Record(_)
            | DefinitionBody::Bits(_)
            | DefinitionBody::Opaque(_)
            | DefinitionBody::Register(_)
            | DefinitionBody::RegisterMap(_)) => (other, None),
        };
        let id = site.owner.derived(&site.identity);
        let name = (self.name)(site);
        if name.is_empty() || self.names.get(&name).is_some_and(|old| old != &id) {
            return Err(TransformError::NameCollision(name));
        }
        self.names.insert(name.clone(), id.clone());
        let reference = Reference::owned(
            member,
            kind,
            Definition::from_body(
                id,
                name,
                match &body {
                    DefinitionBody::Record(v) => v.spec().documentation(),
                    DefinitionBody::Union(v) => v.storage().documentation(),
                    DefinitionBody::Opaque(v) => v.documentation(),
                    DefinitionBody::Register(v) => v.documentation(),
                    DefinitionBody::Bits(_) | DefinitionBody::RegisterMap(_) => {
                        Documentation::EMPTY
                    }
                }
                .with_fallback(fallback),
                body,
            ),
        );
        Ok(match selector {
            Some(selector) => reference.with_selector_binding(selector),
            None => reference,
        })
    }
    pub(super) fn body(
        &mut self,
        body: &DefinitionBody,
        site: &Occurrence,
    ) -> Result<DefinitionBody, TransformError> {
        Ok(match body {
            DefinitionBody::Record(record) => DefinitionBody::Record(self.record(record, site)?),
            DefinitionBody::Union(union) => DefinitionBody::Union(self.union(union, site)?),
            DefinitionBody::RegisterMap(map) => DefinitionBody::RegisterMap(self.map(map, site)?),
            DefinitionBody::Register(register) => {
                DefinitionBody::Register(self.register(register, site)?)
            }
            DefinitionBody::Bits(_) | DefinitionBody::Opaque(_) => body.clone(),
        })
    }
}
