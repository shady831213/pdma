use super::{
    CHeaderExportError, CRecordBackend, Context, ResolvedMemberKind, accessors, analysis, naming,
};
impl CRecordBackend<'_> {
    pub(in super::super) fn opaque_storage(
        &mut self,
        ctx: &Context<'_>,
    ) -> Result<(), CHeaderExportError> {
        let schema = analysis::opaque_schema(ctx)?;
        naming::check_record(schema)?;
        if ctx.ancestors().is_empty() {
            self.output.documentation("", schema.documentation())?;
            self.output.constants::<64>(
                &self.prefix,
                &[("SIZE", schema.size()), ("ALIGNMENT", schema.alignment())],
            )?;
            self.output
                .constants::<32>(&self.prefix, &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)])?;
        }
        for (member, (path, offset)) in schema
            .members()
            .iter()
            .zip(self.analysis.opaque_storage(ctx)?)
        {
            let name = naming::c_identifier(path);
            match member.kind() {
                ResolvedMemberKind::Record { definition, .. } => {
                    self.output
                        .documentation(member.path(), member.documentation())?;
                    self.output.constants::<64>(
                        &format!("{}_RECORD_{name}", self.prefix),
                        &[("OFFSET", *offset), ("SIZE", definition.size())],
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
                        .constants::<64>(&metadata, &[("OFFSET", *offset)])?;
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
}
