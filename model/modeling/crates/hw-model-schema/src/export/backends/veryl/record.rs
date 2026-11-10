use super::VerylRecordBackend;
use crate::export::{
    record::{Backend, Context},
    value::Emit as _,
};
use std::io::Write;
impl Backend for VerylRecordBackend {
    fn entry(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Self::Error> {
        self.output
            .begin(ctx.entry_id(), ctx.entry_name(), writer)?;
        if let Some((size, alignment)) = ctx.entry_extent() {
            self.output.package.extent(size, alignment, writer)?;
        }
        self.emit(ctx.current(), writer)?;
        writer.write_all(b"}\n")?;
        Ok(())
    }
}
