mod accessors;

use super::{CHeaderExportError, CRecordBackend, fragment::Fragment, naming};
use crate::analysis::Layout;
use crate::ast::{RecordDefinition, UnionDefinition};
use crate::export::value::Backend as _;
use crate::export::{
    Artifact,
    record::{Backend, EmitContext},
};
use crate::{EnumDefinition, ResolvedMemberKind};
type CResult<T = ()> = Result<T, CHeaderExportError>;
impl Backend for CRecordBackend {
    fn root(&mut self, ctx: EmitContext<'_>) -> CResult {
        (self.logical_name, self.prefix) = self.names.unit(ctx.unit())?;
        self.output = Fragment::default();
        self.emit(ctx.current())?;
        self.artifacts.push(Artifact {
            logical_name: self.logical_name.clone(),
            media_type: "text/x-c; charset=utf-8".into(),
            contents: std::mem::take(&mut self.output.text).into_bytes(),
        });
        Ok(())
    }
    fn record(&mut self, _: &RecordDefinition, ctx: EmitContext<'_>) -> CResult {
        self.storage(&ctx)
    }
    fn union(&mut self, _: &UnionDefinition, ctx: EmitContext<'_>) -> CResult {
        self.storage(&ctx)
    }
}

impl CRecordBackend {
    pub(super) fn storage(&mut self, ctx: &EmitContext<'_>) -> Result<(), CHeaderExportError> {
        let Layout::Record(schema) = &ctx.unit().body else {
            return Err(CHeaderExportError::InvalidVisit("record layout"));
        };
        naming::check_record(schema)?;
        self.output.documentation("", schema.documentation())?;
        self.output.constants::<64>(
            &self.prefix,
            &[("SIZE", schema.size()), ("ALIGNMENT", schema.alignment())],
        )?;
        self.output
            .constants::<32>(&self.prefix, &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)])?;
        for member in schema.members() {
            let name = naming::c_identifier(naming::relative_path(schema.root(), member.path()));
            match member.kind() {
                ResolvedMemberKind::Record { definition, .. } => {
                    self.output
                        .documentation(member.path(), member.documentation())?;
                    self.output.constants::<64>(
                        &format!("{}_RECORD_{name}", self.prefix),
                        &[
                            ("OFFSET", member.absolute_offset()),
                            ("SIZE", definition.size()),
                        ],
                    )?;
                }
                ResolvedMemberKind::Field { definition, .. } => {
                    self.output.text.push('\n');
                    self.output.documentation("", member.documentation())?;
                    let metadata = format!("{}_{name}", self.prefix);
                    if let Some(enumeration) = definition.enum_definition() {
                        self.enum_constants(&metadata, enumeration)?;
                    }
                    for bit in definition.bits() {
                        self.output.documentation(bit.name(), bit.documentation())?;
                        if let Some(enumeration) = bit.definition().enum_definition() {
                            self.enum_constants(
                                &format!("{metadata}_{}", naming::c_identifier(bit.name())),
                                enumeration,
                            )?;
                        }
                    }
                    self.output
                        .constants::<64>(&metadata, &[("OFFSET", member.absolute_offset())])?;
                    let names = accessors::FieldNames {
                        prefix: &self.prefix,
                        field: &name,
                        metadata: &metadata,
                    };
                    accessors::scalar_accessors(&mut self.output, names, definition)?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn enum_constants(
        &mut self,
        prefix: &str,
        enumeration: EnumDefinition,
    ) -> Result<(), CHeaderExportError> {
        for variant in enumeration.variants() {
            self.output
                .documentation(variant.name(), variant.documentation())?;
            self.output.unsigned::<64>(
                &format!("{prefix}_{}", naming::c_identifier(variant.name())),
                variant.value(),
            )?;
        }
        Ok(())
    }
}
