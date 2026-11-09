use super::{Occurrence, TransformError, engine::Engine};
use crate::ast::{
    CaseBody, DefinitionBody, FieldType, RecordDefinition, ReferenceKind, UnionDefinition,
};
impl<S, N> Engine<'_, S, N>
where
    S: FnMut(&Occurrence) -> bool,
    N: FnMut(&Occurrence) -> String,
{
    pub(super) fn record(
        &mut self,
        record: &RecordDefinition,
        owner: &Occurrence,
    ) -> Result<RecordDefinition, TransformError> {
        let fields = record
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let site = owner.child("field", index, field.name());
                let mut documentation = field.documentation();
                let inline = match field.ty() {
                    FieldType::Reference(reference) => {
                        let reference = self.reference(reference)?;
                        if self.outline || !self.selected(&site) {
                            return Ok(field.with_type(FieldType::Reference(reference)));
                        }
                        let target = Self::target(&reference, &site)?;
                        documentation = documentation.with_fallback(target.documentation());
                        target.body().clone()
                    }
                    FieldType::Scalar { .. } | FieldType::Enum(_) => return Ok(field.clone()),
                    FieldType::Bits(bits) => DefinitionBody::Bits(bits.clone()),
                    FieldType::Record(record) => {
                        DefinitionBody::Record(self.record(record, &site)?)
                    }
                    FieldType::Union(union) => DefinitionBody::Union(self.union(union, &site)?),
                    FieldType::Opaque(spec) => DefinitionBody::Opaque(*spec),
                };
                let ty = if self.outline && self.selected(&site) {
                    FieldType::Reference(self.lift(
                        inline,
                        &site,
                        field.name(),
                        ReferenceKind::Field,
                        documentation,
                    )?)
                } else {
                    field_type(inline, &site)?
                };
                Ok(field.with_type(ty).with_documentation(documentation))
            })
            .collect::<Result<_, _>>()?;
        Ok(RecordDefinition::new(record.spec(), fields)
            .with_declaration(record.declared_alignment(), record.endian()))
    }
    pub(super) fn union(
        &mut self,
        union: &UnionDefinition,
        owner: &Occurrence,
    ) -> Result<UnionDefinition, TransformError> {
        let cases = union
            .cases()
            .iter()
            .enumerate()
            .map(|(index, case)| {
                let site = owner.child("case", index, case.name());
                let inline = match case.body() {
                    CaseBody::Reference(reference) => {
                        let reference = self.reference(reference)?;
                        if self.outline || !self.selected(&site) {
                            return Ok(case.with_body(CaseBody::Reference(reference)));
                        }
                        Self::target(&reference, &site)?.body().clone()
                    }
                    CaseBody::Inline(record) => DefinitionBody::Record(self.record(record, &site)?),
                    CaseBody::Bits(bits) => DefinitionBody::Bits(bits.clone()),
                    CaseBody::Union(union) => DefinitionBody::Union(self.union(union, &site)?),
                    CaseBody::Opaque(spec) => DefinitionBody::Opaque(*spec),
                };
                let body = if self.outline && self.selected(&site) {
                    let mut reference = self.lift(
                        inline,
                        &site,
                        case.name(),
                        ReferenceKind::Variant,
                        crate::Documentation::EMPTY,
                    )?;
                    if let Some(value) = case.discriminant() {
                        reference = reference.with_discriminant(value);
                    }
                    CaseBody::Reference(reference)
                } else {
                    case_body(inline, &site)?
                };
                Ok(case.with_body(body))
            })
            .collect::<Result<_, _>>()?;
        Ok(UnionDefinition::new(
            union.storage(),
            union.selector(),
            cases,
        ))
    }
}
fn field_type(body: DefinitionBody, site: &Occurrence) -> Result<FieldType, TransformError> {
    match body {
        DefinitionBody::Bits(bits) => Ok(FieldType::Bits(bits)),
        DefinitionBody::Record(record) => Ok(FieldType::Record(Box::new(record))),
        DefinitionBody::Union(union) => Ok(FieldType::Union(Box::new(union))),
        DefinitionBody::Opaque(spec) => Ok(FieldType::Opaque(spec)),
        DefinitionBody::Register(_) | DefinitionBody::RegisterMap(_) => {
            Err(TransformError::Unsupported {
                path: site.path.clone(),
            })
        }
    }
}
fn case_body(body: DefinitionBody, site: &Occurrence) -> Result<CaseBody, TransformError> {
    match body {
        DefinitionBody::Bits(bits) => Ok(CaseBody::Bits(bits)),
        DefinitionBody::Record(record) => Ok(CaseBody::Inline(record)),
        DefinitionBody::Union(union) => Ok(CaseBody::Union(Box::new(union))),
        DefinitionBody::Opaque(spec) => Ok(CaseBody::Opaque(spec)),
        DefinitionBody::Register(_) | DefinitionBody::RegisterMap(_) => {
            Err(TransformError::Unsupported {
                path: site.path.clone(),
            })
        }
    }
}
