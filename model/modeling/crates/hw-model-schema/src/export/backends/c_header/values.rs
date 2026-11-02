use super::{CHeaderExportError, CRecordBackend, CRegisterBackend, fragment::Fragment, naming};
use crate::ast::BitsDefinition;
use crate::export::{
    Artifact, record, register,
    value::{Backend, DocumentationKind, DocumentationOwner},
};
use crate::{BitsFieldSpec, EnumVariantSpec, RecordSpec};
type CResult<T = ()> = Result<T, CHeaderExportError>;

impl Backend for CRecordBackend {
    type Domain = record::Domain;
    type Error = CHeaderExportError;
    type Context<'a> = record::EmitContext<'a>;
    fn short_description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> CResult {
        if matches!(
            owner.kind,
            DocumentationKind::Record | DocumentationKind::Union | DocumentationKind::Opaque
        ) {
            return Ok(());
        }
        self.output.comment(owner.name, text)
    }
    fn description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
    ) -> CResult {
        self.short_description(text, owner, ctx)
    }

    fn bits(&mut self, bits: &BitsDefinition, ctx: Self::Context<'_>) -> CResult {
        bits_metadata(&mut self.output, &self.prefix, bits)?;
        self.emit_all(ctx.children())
    }
    fn bit_field(&mut self, field: &BitsFieldSpec, ctx: Self::Context<'_>) -> CResult {
        bit_metadata(&mut self.output, &self.prefix, field)?;
        self.emit_all(ctx.children())
    }
    fn variant(&mut self, variant: &EnumVariantSpec, ctx: Self::Context<'_>) -> CResult {
        let prefix = value_prefix(&self.prefix, ctx.bit());
        self.output.unsigned::<64>(
            &format!("{prefix}_{}", naming::c_identifier(variant.name())),
            variant.value(),
        )
    }
    fn opaque(&mut self, _: &RecordSpec, ctx: Self::Context<'_>) -> CResult {
        self.storage(&ctx)
    }
    fn finish(self) -> CResult<Vec<Artifact>> {
        Ok(self.artifacts)
    }
}
impl Backend for CRegisterBackend {
    type Domain = register::Domain;
    type Error = CHeaderExportError;
    type Context<'a> = register::EmitContext<'a>;
    fn short_description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
    ) -> CResult {
        use crate::{
            analysis::{ConcretePlacement, Placement},
            ast::RegisterLocation,
        };
        if matches!(
            ctx.placement(),
            Some(Placement::Concrete(
                ConcretePlacement::Register {
                    location: RegisterLocation::Instance { .. },
                    ..
                } | ConcretePlacement::Bank { expanded: true, .. }
            ))
        ) {
            return Ok(());
        }
        self.output.comment(owner.name, text)
    }
    fn description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
    ) -> CResult {
        self.short_description(text, owner, ctx)
    }
    fn bits(&mut self, bits: &BitsDefinition, ctx: Self::Context<'_>) -> CResult {
        bits_metadata(&mut self.output, &self.prefix, bits)?;
        self.emit_all(ctx.children())
    }
    fn bit_field(&mut self, field: &BitsFieldSpec, ctx: Self::Context<'_>) -> CResult {
        bit_metadata(&mut self.output, &self.prefix, field)?;
        self.emit_all(ctx.children())
    }
    fn variant(&mut self, variant: &EnumVariantSpec, ctx: Self::Context<'_>) -> CResult {
        let prefix = if ctx.register().is_some() {
            self.register_name(ctx.register_location())?
        } else {
            self.prefix.clone()
        };
        let prefix = value_prefix(&prefix, ctx.bit());
        self.output.unsigned::<64>(
            &format!("{prefix}_{}", naming::c_identifier(variant.name())),
            variant.value(),
        )
    }
    fn opaque(&mut self, spec: &RecordSpec, _: Self::Context<'_>) -> CResult {
        self.output.constants::<64>(
            &self.prefix,
            &[("SIZE", spec.size()), ("ALIGNMENT", spec.alignment())],
        )?;
        self.output
            .constants::<32>(&self.prefix, &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)])
    }
    fn finish(self) -> CResult<Vec<Artifact>> {
        Ok(self.artifacts)
    }
}
fn bits_metadata(output: &mut Fragment, prefix: &str, bits: &BitsDefinition) -> CResult {
    output.constants::<64>(prefix, &[("SIZE", bits.scalar().width())])?;
    output.constants::<32>(prefix, &[("BIT_WIDTH", bits.bit_width())])?;
    output.constants::<64>(prefix, &[("DECLARED_MASK", bits.declared_mask())])
}
fn bit_metadata(output: &mut Fragment, prefix: &str, field: &BitsFieldSpec) -> CResult {
    let prefix = format!("{prefix}_{}", naming::c_identifier(field.name()));
    output.constants::<32>(
        &prefix,
        &[("LSB", field.lsb()), ("BITS", field.bit_width())],
    )
}
fn value_prefix(prefix: &str, bit: Option<&str>) -> String {
    bit.map_or_else(
        || prefix.into(),
        |bit| format!("{prefix}_{}", naming::c_identifier(bit)),
    )
}
