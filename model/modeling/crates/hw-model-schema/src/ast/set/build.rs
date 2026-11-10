//! Canonical construction shared by macro output and hand-built declarations.
mod record;
mod register;
use super::GraphError;
use crate::{
    Documentation,
    ast::{Definition, DefinitionBody, DefinitionId, Origin, Reference, ReferenceKind},
};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Collector {
    pub(super) order: Vec<DefinitionId>,
    pub(super) complete: HashMap<DefinitionId, Definition>,
    active: HashMap<DefinitionId, usize>,
    paths: HashMap<(&'static str, Vec<String>), DefinitionId>,
}

impl Collector {
    pub(super) fn visit(
        &mut self,
        definition: &Definition,
        path: &mut Vec<String>,
    ) -> Result<Definition, GraphError> {
        let id = definition.id();
        if let Some(complete) = self.complete.get(&id) {
            return Ok(complete.clone());
        }
        if let Some(start) = self.active.insert(id.clone(), path.len()) {
            let mut cycle: Vec<_> = path.iter().skip(start).cloned().collect();
            cycle.push(id.path().to_owned());
            return Err(GraphError::Cycle(cycle));
        }
        if self
            .paths
            .insert((id.path(), id.structural_path().to_vec()), id.clone())
            .is_some_and(|previous| previous != id)
        {
            return Err(GraphError::AmbiguousPath(id.path().to_owned()));
        }
        let body = self.body(definition, path)?;
        let references = definition
            .extra_references()
            .iter()
            .map(|r| self.reference(&definition.id(), r, path))
            .collect::<Result<_, _>>()?;
        let definition = definition.rewritten(body, references);
        self.active.remove(&id);
        self.complete.insert(id.clone(), definition.clone());
        self.order.push(id);
        Ok(definition)
    }

    fn reference(
        &mut self,
        owner: &DefinitionId,
        reference: &Reference,
        path: &mut Vec<String>,
    ) -> Result<Reference, GraphError> {
        path.push(format!("{}.{}", owner.path(), reference.name()));
        let target = self.visit(&reference.target(), path)?;
        path.pop();
        Ok(reference.retarget(target))
    }

    fn lift(
        &mut self,
        owner: &Definition,
        site: (ReferenceKind, usize, &'static str),
        body: DefinitionBody,
        documentation: Documentation,
        path: &mut Vec<String>,
    ) -> Result<Reference, GraphError> {
        let (kind, index, name) = site;
        let role = match kind {
            ReferenceKind::Field => "field",
            ReferenceKind::Variant => "case",
            ReferenceKind::Placement => "placement",
        };
        let documentation = match &body {
            DefinitionBody::Record(v) => v.spec().documentation(),
            DefinitionBody::Union(v) => v.storage().documentation(),
            DefinitionBody::Opaque(v) => v.documentation(),
            DefinitionBody::Register(v) => v.documentation(),
            DefinitionBody::Enumeration(crate::ast::value::Enumeration::Scalar(v)) => {
                v.documentation()
            }
            _ => Documentation::EMPTY,
        }
        .with_fallback(documentation);
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
            other => (other, None),
        };
        let definition = Definition::from_body(
            owner.id().derived(&[format!("{role}:{index}:{name}")]),
            format!("{}_{name}", owner.name()),
            documentation,
            body,
        )
        .with_origin(Origin::Inline)
        .at_member(name);
        let reference = Reference::owned(name, kind, definition);
        let reference = match selector {
            Some(selector) => reference.with_selector_binding(selector),
            None => reference,
        };
        self.reference(&owner.id(), &reference, path)
    }

    fn body(
        &mut self,
        definition: &Definition,
        path: &mut Vec<String>,
    ) -> Result<DefinitionBody, GraphError> {
        Ok(match definition.body() {
            DefinitionBody::Record(record) => {
                DefinitionBody::Record(self.record(definition, record, path)?)
            }
            DefinitionBody::Union(union) => {
                DefinitionBody::Union(self.union(definition, union, path)?)
            }
            DefinitionBody::RegisterMap(map) => {
                DefinitionBody::RegisterMap(self.map(definition, map, path)?)
            }
            DefinitionBody::Register(register) => {
                DefinitionBody::Register(self.register(definition, register, path)?)
            }
            DefinitionBody::Bits(bits) => DefinitionBody::Bits(
                bits.with_enum_references(self.enums(definition, bits.members(), path)?),
            ),
            body @ (DefinitionBody::Enumeration(_) | DefinitionBody::Opaque(_)) => body.clone(),
        })
    }
}
