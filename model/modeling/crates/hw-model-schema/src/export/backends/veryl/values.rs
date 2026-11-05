use super::{VerylExportError, VerylRecordBackend, VerylRegisterBackend, package::register_name};
use crate::{
    EnumVariantSpec, RecordSpec,
    ast::BitsDefinition,
    export::{
        Artifact, record, register,
        value::{self, Backend},
    },
};
impl Backend for VerylRecordBackend {
    type Error = VerylExportError;
    type Domain = record::Domain;
    type Context<'a> = crate::traversal::record::Context<'a>;
    fn bits<'a>(
        &mut self,
        bits: &'a BitsDefinition,
        ctx: Self::Context<'a>,
    ) -> Result<(), Self::Error> {
        if ctx.field().is_none() {
            self.output
                .package
                .extent(bits.scalar().width(), bits.scalar().width())?;
            self.output
                .package
                .constant("BIT_WIDTH", "u32", bits.bit_width())?;
            self.output.package.constant(
                "DECLARED_MASK",
                "u64",
                format_args!("64'h{:016x}", bits.declared_mask()),
            )?;
        }
        self.emit_all(ctx.children())
    }
    fn opaque(&mut self, _: &RecordSpec, ctx: Self::Context<'_>) -> Result<(), Self::Error> {
        self.extent(&ctx)
    }
    fn short_description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.output.package.comment(text)
    }
    fn description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.output.package.comment(text)
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(self.output.artifacts)
    }
}
impl Backend for VerylRegisterBackend {
    type Error = VerylExportError;
    type Domain = register::Domain;
    type Context<'a> = crate::traversal::register::Context<'a>;
    fn bits<'a>(
        &mut self,
        bits: &'a BitsDefinition,
        ctx: Self::Context<'a>,
    ) -> Result<(), Self::Error> {
        self.output
            .package
            .extent(bits.scalar().width(), bits.scalar().width())?;
        self.output
            .package
            .constant("BIT_WIDTH", "u32", bits.bit_width())?;
        self.output.package.constant(
            "DECLARED_MASK",
            "u64",
            format_args!("64'h{:016x}", bits.declared_mask()),
        )?;
        self.emit_all(ctx.children())
    }
    fn opaque(&mut self, value: &RecordSpec, _: Self::Context<'_>) -> Result<(), Self::Error> {
        self.output.package.extent(value.size(), value.alignment())
    }
    fn variant(
        &mut self,
        variant: &EnumVariantSpec,
        ctx: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        if let (Some(bit), Some(_)) = (ctx.bit(), ctx.register()) {
            let prefix = register_name(ctx.register_segments());
            self.output.package.constant(
                &format!(
                    "{prefix}_{}_{}",
                    bit.to_ascii_uppercase(),
                    variant.name().to_ascii_uppercase()
                ),
                "u64",
                variant.value(),
            )?;
        }
        Ok(())
    }
    fn short_description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.output.package.comment(text)
    }
    fn description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.output.package.comment(text)
    }
    fn finish(self) -> Result<Vec<Artifact>, Self::Error> {
        Ok(self.output.artifacts)
    }
}
