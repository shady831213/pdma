use super::{VerylExportError, identifier};
use crate::export::SymbolScope;
use std::fmt::Display;
use std::io::Write;

#[derive(Debug, Default)]
pub(super) struct Package {
    symbols: SymbolScope<()>,
}
impl Package {
    pub(super) fn comment(text: &str, writer: &mut dyn Write) -> Result<(), VerylExportError> {
        for line in text.lines() {
            writeln!(
                writer,
                "    // {}",
                line.replace(['\r', '\u{2028}', '\u{2029}'], " ")
            )?;
        }
        Ok(())
    }
    pub(super) fn declare(&mut self, name: &str) -> Result<(), VerylExportError> {
        if !identifier(name) || self.symbols.insert(name, ()).is_err() {
            return Err(VerylExportError::Name(name.into()));
        }
        Ok(())
    }
    pub(super) fn constant(
        &mut self,
        name: &str,
        ty: &str,
        value: impl Display,
        writer: &mut dyn Write,
    ) -> Result<(), VerylExportError> {
        self.declare(name)?;
        writeln!(writer, "    const {name}: {ty} = {value};")?;
        Ok(())
    }
    pub(super) fn extent(
        &mut self,
        extent: usize,
        alignment: usize,
        writer: &mut dyn Write,
    ) -> Result<(), VerylExportError> {
        self.constant("EXTENT", "u64", extent, writer)?;
        self.constant("ALIGNMENT", "u64", alignment, writer)
    }
}

pub(super) fn register_name(segments: &[crate::analysis::ViewPathSegment]) -> String {
    if segments.is_empty() {
        return "VALUE".into();
    }
    segments
        .iter()
        .map(|part| {
            let name = part.name.to_ascii_uppercase();
            part.index
                .map_or_else(|| name.clone(), |index| format!("{name}_{index}"))
        })
        .collect::<Vec<_>>()
        .join("_")
}
