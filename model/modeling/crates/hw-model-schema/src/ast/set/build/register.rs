use super::Collector;
use crate::ast::{Definition, GraphError, MapDefinition, RegisterInfo};
impl Collector {
    pub(super) fn enums<'a, I: Copy + 'static>(
        &mut self,
        owner: &Definition,
        fields: impl Iterator<Item = crate::ast::value::BitField<'a, I>> + 'a,
        path: &mut Vec<String>,
    ) -> Result<Vec<Option<crate::ast::Reference>>, GraphError> {
        fields
            .enumerate()
            .map(|(index, field)| {
                if let Some(reference) = field.reference() {
                    return self.reference(&owner.id(), reference, path).map(Some);
                }
                if field.definition().enum_definition().is_none() {
                    return Ok(None);
                }
                self.lift(
                    owner,
                    (crate::ast::ReferenceKind::Field, index, field.name()),
                    crate::ast::DefinitionBody::Enumeration(crate::ast::value::Enumeration::Bits(
                        field.definition(),
                    )),
                    field.documentation(),
                    path,
                )
                .map(Some)
            })
            .collect()
    }
    pub(super) fn map(
        &mut self,
        owner: &Definition,
        map: &MapDefinition,
        path: &mut Vec<String>,
    ) -> Result<MapDefinition, GraphError> {
        let placements = map
            .placements()
            .iter()
            .map(|placement| {
                Ok(placement.with_reference(self.reference(
                    &owner.id(),
                    placement.reference(),
                    path,
                )?))
            })
            .collect::<Result<_, GraphError>>()?;
        Ok(MapDefinition::new(
            map.parameters(),
            map.alignment(),
            map.explicit_extent(),
            placements,
        ))
    }
    pub(super) fn register(
        &mut self,
        owner: &Definition,
        register: &RegisterInfo,
        path: &mut Vec<String>,
    ) -> Result<RegisterInfo, GraphError> {
        if let Some(reference) = register.source() {
            return Ok(register.clone().with_source(self.reference(
                &owner.id(),
                reference,
                path,
            )?));
        }
        Ok(register.with_enum_references(self.enums(owner, register.members(), path)?))
    }
}
