use super::{
    CHeaderExportError, CRecordBackend, Context, ResolvedMemberKind, accessors, analysis, naming,
};
use crate::export::backends::c_header::fragment::Fragment;
use std::io::Write;
impl CRecordBackend<'_> {
    pub(in super::super) fn opaque_storage(
        &mut self,
        ctx: &Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        let schema = analysis::opaque_schema(ctx)?;
        naming::check_record(schema)?;
        if ctx.ancestors().is_empty() {
            Fragment::documentation("", schema.documentation(), writer)?;
            self.output.constants::<64>(
                &self.prefix,
                &[("SIZE", schema.size()), ("ALIGNMENT", schema.alignment())],
                writer,
            )?;
            self.output.constants::<32>(
                &self.prefix,
                &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)],
                writer,
            )?;
        }
        for (member, (path, offset)) in schema
            .members()
            .iter()
            .zip(self.analysis.opaque_storage(ctx)?)
        {
            let name = naming::c_identifier(path);
            match member.kind() {
                ResolvedMemberKind::Record { definition, .. } => {
                    Fragment::documentation(member.path(), member.documentation(), writer)?;
                    self.output.constants::<64>(
                        &format!("{}_RECORD_{name}", self.prefix),
                        &[("OFFSET", *offset), ("SIZE", definition.size())],
                        writer,
                    )?;
                }
                ResolvedMemberKind::Field { definition, .. } => {
                    writer.write_all(b"\n")?;
                    Fragment::documentation("", member.documentation(), writer)?;
                    let metadata = format!("{}_{name}", self.prefix);
                    if let Some(enumeration) = definition.enum_definition() {
                        self.enum_constants(&metadata, enumeration, writer)?;
                    }
                    for bit in definition.bits() {
                        Fragment::documentation(bit.name(), bit.documentation(), writer)?;
                        if let Some(enumeration) = bit.definition().enum_definition() {
                            self.enum_constants(
                                &format!("{metadata}_{}", naming::c_identifier(bit.name())),
                                enumeration,
                                writer,
                            )?;
                        }
                    }
                    self.output
                        .constants::<64>(&metadata, &[("OFFSET", *offset)], writer)?;
                    let names = accessors::FieldNames {
                        prefix: &self.prefix,
                        field: &name,
                        metadata: &metadata,
                    };
                    accessors::scalar_accessors(&mut self.output, names, definition, writer)?;
                }
            }
        }
        Ok(())
    }
}
