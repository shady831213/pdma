use super::{CHeaderBackend, CHeaderExportError, naming, record};
use crate::ast::analysis::Layout;
use crate::export::Scope;
use crate::{EnumDefinition, ResolvedMemberKind};

impl CHeaderBackend {
    pub(super) fn storage(&mut self, scope: Scope<'_>) -> Result<(), CHeaderExportError> {
        let Layout::Record(schema) = &scope.unit.body else {
            return Err(CHeaderExportError::InvalidVisit("record layout"));
        };
        naming::check_record(schema)?;
        self.record_root = schema.root();
        self.output.documentation("", schema.documentation())?;
        self.output.constants::<64>(
            &self.prefix,
            &[("SIZE", schema.size()), ("ALIGNMENT", schema.alignment())],
        )?;
        self.output
            .constants::<32>(&self.prefix, &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)])?;
        for member in schema.members() {
            let name = self.record_member_name(member);
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
                    let names = record::FieldNames {
                        prefix: &self.prefix,
                        field: &name,
                        metadata: &metadata,
                    };
                    record::scalar_accessors(&mut self.output, names, definition)?;
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
