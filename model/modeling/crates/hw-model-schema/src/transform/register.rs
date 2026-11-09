use super::{Occurrence, TransformError, engine::Engine};
use crate::ast::{DefinitionBody, MapDefinition, PlacementBody, ReferenceKind, RegisterInfo};
impl<S, N> Engine<'_, S, N>
where
    S: FnMut(&Occurrence) -> bool,
    N: FnMut(&Occurrence) -> String,
{
    pub(super) fn map(
        &mut self,
        map: &MapDefinition,
        owner: &Occurrence,
    ) -> Result<MapDefinition, TransformError> {
        let placements = map
            .placements()
            .iter()
            .enumerate()
            .map(|(index, placement)| {
                let site = owner.child("placement", index, placement.name());
                let child = match placement.body() {
                    PlacementBody::Reference(reference) => {
                        let reference = self.reference(reference)?;
                        if self.outline || !self.selected(&site) {
                            return Ok(placement.with_body(PlacementBody::Reference(reference)));
                        }
                        Self::target(&reference, &site)?
                    }
                    PlacementBody::Inline(child) => {
                        child.rewritten(self.body(child.body(), &site)?, Vec::new())
                    }
                };
                match child.body() {
                    DefinitionBody::Register(_)
                    | DefinitionBody::RegisterMap(_)
                    | DefinitionBody::Opaque(_) => {}
                    DefinitionBody::Bits(_)
                    | DefinitionBody::Record(_)
                    | DefinitionBody::Union(_) => {
                        return Err(TransformError::Unsupported { path: site.path });
                    }
                }
                let body = if self.outline && self.selected(&site) {
                    PlacementBody::Reference(self.lift(
                        child.body().clone(),
                        &site,
                        placement.name(),
                        ReferenceKind::Placement,
                        child.documentation(),
                    )?)
                } else {
                    PlacementBody::Inline(Box::new(child))
                };
                Ok(placement.with_body(body))
            })
            .collect::<Result<_, _>>()?;
        Ok(MapDefinition::new(
            map.parameters(),
            map.alignment(),
            map.explicit_extent(),
            placements,
        ))
    }
    pub(super) fn register(
        &mut self,
        register: &RegisterInfo,
        owner: &Occurrence,
    ) -> Result<RegisterInfo, TransformError> {
        let site = owner.child("source", 0, "bits");
        if let Some(reference) = register.source() {
            let reference = self.reference(reference)?;
            if self.outline || !self.selected(&site) {
                return Ok(register.rewritten_source(Some(reference)));
            }
            return match reference.target().body() {
                DefinitionBody::Bits(bits) => Ok(register.with_inline_bits(bits.clone())),
                DefinitionBody::Record(_)
                | DefinitionBody::Union(_)
                | DefinitionBody::Opaque(_)
                | DefinitionBody::Register(_)
                | DefinitionBody::RegisterMap(_) => {
                    Err(TransformError::Unsupported { path: site.path })
                }
            };
        }
        if let Some(bits) = register.inline_bits()
            && self.outline
            && self.selected(&site)
        {
            return Ok(register.rewritten_source(Some(self.lift(
                DefinitionBody::Bits(bits.clone()),
                &site,
                "bits",
                ReferenceKind::Field,
                crate::Documentation::EMPTY,
            )?)));
        }
        Ok(register.clone())
    }
}
