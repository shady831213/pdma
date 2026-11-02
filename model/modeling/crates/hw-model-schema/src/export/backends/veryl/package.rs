use super::{VerylExportError, identifier};
use crate::export::SymbolScope;
use std::fmt::{Display, Write};

#[derive(Debug, Default)]
pub(super) struct Package {
    pub(super) text: String,
    symbols: SymbolScope<()>,
}
impl Package {
    pub(super) fn comment(&mut self, text: &str) -> Result<(), VerylExportError> {
        for line in text.lines() {
            writeln!(
                self.text,
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
    ) -> Result<(), VerylExportError> {
        self.declare(name)?;
        writeln!(self.text, "    const {name}: {ty} = {value};")?;
        Ok(())
    }
    pub(super) fn extent(
        &mut self,
        extent: usize,
        alignment: usize,
    ) -> Result<(), VerylExportError> {
        self.constant("EXTENT", "u64", extent)?;
        self.constant("ALIGNMENT", "u64", alignment)
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
