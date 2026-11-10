use super::Collector;
use crate::ast::{
    CaseBody, Definition, DefinitionBody, FieldType, GraphError, RecordDefinition, ReferenceKind,
    UnionDefinition, value::Enumeration,
};
impl Collector {
    pub(super) fn record(
        &mut self,
        owner: &Definition,
        record: &RecordDefinition,
        path: &mut Vec<String>,
    ) -> Result<RecordDefinition, GraphError> {
        let fields = record
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let body = match field.ty() {
                    FieldType::Scalar { .. } => return Ok(field.clone()),
                    FieldType::Reference(reference) => {
                        return Ok(field.with_type(FieldType::Reference(self.reference(
                            &owner.id(),
                            reference,
                            path,
                        )?)));
                    }
                    FieldType::Enum(value) => {
                        DefinitionBody::Enumeration(Enumeration::Scalar(*value))
                    }
                    FieldType::Bits(value) => DefinitionBody::Bits(value.clone()),
                    FieldType::Union(value) => DefinitionBody::Union(value.as_ref().clone()),
                    FieldType::Record(value) => DefinitionBody::Record(value.as_ref().clone()),
                    FieldType::Opaque(value) => DefinitionBody::Opaque(*value),
                };
                let reference = self.lift(
                    owner,
                    (ReferenceKind::Field, index, field.name()),
                    body,
                    field.documentation(),
                    path,
                )?;
                Ok(field.with_type(FieldType::Reference(reference)))
            })
            .collect::<Result<_, GraphError>>()?;
        Ok(RecordDefinition::new(record.spec(), fields)
            .with_declaration(record.declared_alignment(), record.endian()))
    }

    pub(super) fn union(
        &mut self,
        owner: &Definition,
        union: &UnionDefinition,
        path: &mut Vec<String>,
    ) -> Result<UnionDefinition, GraphError> {
        let cases = union
            .cases()
            .iter()
            .enumerate()
            .map(|(index, case)| {
                let body = match case.body() {
                    CaseBody::Reference(reference) => {
                        return Ok(case.with_body(CaseBody::Reference(self.reference(
                            &owner.id(),
                            reference,
                            path,
                        )?)));
                    }
                    CaseBody::Inline(value) => DefinitionBody::Record(value.clone()),
                    CaseBody::Bits(value) => DefinitionBody::Bits(value.clone()),
                    CaseBody::Union(value) => DefinitionBody::Union(value.as_ref().clone()),
                    CaseBody::Opaque(value) => DefinitionBody::Opaque(*value),
                };
                let mut reference = self.lift(
                    owner,
                    (ReferenceKind::Variant, index, case.name()),
                    body,
                    crate::Documentation::EMPTY,
                    path,
                )?;
                if let Some(value) = case.discriminant() {
                    reference = reference.with_discriminant(value);
                }
                Ok(case.with_body(CaseBody::Reference(reference)))
            })
            .collect::<Result<_, GraphError>>()?;
        Ok(UnionDefinition::new(
            union.storage(),
            union.selector(),
            cases,
        ))
    }
}
