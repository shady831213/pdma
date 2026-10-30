use super::{VerylExportError, identifier};
use crate::{
    Access, Endian,
    ast::{PlacementView, RegisterInfo, RegisterLocation},
    export::{Scope, SymbolScope},
};
use std::fmt::{Display, Write};

#[derive(Debug, Default)]
pub(super) struct Package {
    pub(super) text: String,
    symbols: SymbolScope<()>,
}
impl Package {
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
    pub(super) fn register(
        &mut self,
        prefix: &str,
        offset: usize,
        register: &RegisterInfo,
    ) -> Result<(), VerylExportError> {
        if !matches!(register.width(), 1 | 2 | 4 | 8) {
            return Err(VerylExportError::Register(prefix.into()));
        }
        let (read, write) = match register.access() {
            Access::ReadOnly => (true, false),
            Access::WriteOnly => (false, true),
            Access::ReadWrite => (true, true),
        };
        self.constant(
            &format!("{prefix}_ENDIAN_LITTLE"),
            "bbool",
            register.endian() == Endian::Little,
        )?;
        if let Some(description) = register.documentation().short_description {
            for line in description.lines() {
                writeln!(self.text, "    // {line}")?;
            }
        }
        self.constant(&format!("{prefix}_OFFSET"), "u64", offset)?;
        self.constant(&format!("{prefix}_BYTES"), "u32", register.width())?;
        self.constant(
            &format!("{prefix}_RESET"),
            "u64",
            format_args!("64'h{:016x}", register.reset_value()),
        )?;
        self.constant(
            &format!("{prefix}_WRITABLE"),
            "u64",
            format_args!("64'h{:016x}", register.writable_mask()),
        )?;
        self.constant(&format!("{prefix}_READ"), "bbool", read)?;
        self.constant(&format!("{prefix}_WRITE"), "bbool", write)?;
        Ok(())
    }
}

pub(super) fn prefix(scope: Scope<'_>) -> Option<String> {
    match scope.register_location() {
        RegisterLocation::Root => Some("VALUE".into()),
        RegisterLocation::Template { .. } => None,
        RegisterLocation::Standalone(_) | RegisterLocation::Instance { .. } => {
            match scope.placement {
                Some(PlacementView::Register { view: leaf, .. }) => Some({
                    leaf.segments
                        .iter()
                        .map(|part| {
                            let name = part.name.to_ascii_uppercase();
                            part.index
                                .map_or_else(|| name.clone(), |index| format!("{name}_{index}"))
                        })
                        .collect::<Vec<_>>()
                        .join("_")
                }),
                Some(PlacementView::Declared(_) | PlacementView::Bank { .. }) | None => None,
            }
        }
    }
}
