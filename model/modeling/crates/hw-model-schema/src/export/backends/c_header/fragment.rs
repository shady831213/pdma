use std::fmt::{Display, Write as _};

use crate::Documentation;

use super::{CHeaderExportError, naming};

#[derive(Debug, Default)]
pub(super) struct Fragment {
    pub(super) text: String,
    symbols: naming::Scope,
}

impl Fragment {
    pub(super) fn define(
        &mut self,
        name: &str,
        value: impl Display,
    ) -> Result<(), CHeaderExportError> {
        self.symbols.insert(name, name)?;
        writeln!(self.text, "#define {name} {value}").map_err(CHeaderExportError::Format)
    }

    pub(super) fn constants<const WIDTH: usize>(
        &mut self,
        prefix: &str,
        values: &[(&str, impl Display)],
    ) -> Result<(), CHeaderExportError> {
        for (name, value) in values {
            self.unsigned::<WIDTH>(&format!("{prefix}_{name}"), value)?;
        }
        Ok(())
    }

    pub(super) fn unsigned<const WIDTH: usize>(
        &mut self,
        name: &str,
        value: impl Display,
    ) -> Result<(), CHeaderExportError> {
        self.define(name, format_args!("UINT{WIDTH}_C({value})"))
    }

    pub(super) fn line(&mut self, line: impl Display) -> Result<(), CHeaderExportError> {
        writeln!(self.text, "{line}").map_err(CHeaderExportError::Format)
    }

    pub(super) fn documentation(
        &mut self,
        name: &str,
        docs: Documentation,
    ) -> Result<(), CHeaderExportError> {
        if docs == Documentation::EMPTY {
            return Ok(());
        }
        self.text.push_str("/*\n");
        if !name.is_empty() {
            self.comment_text(name)?;
        }
        for fragment in crate::export::value::documentation_fragments(docs) {
            let (crate::export::value::DocumentationFragment::ShortDescription(text)
            | crate::export::value::DocumentationFragment::Description(text)) = fragment;
            self.comment_text(text)?;
        }
        self.line(" */")
    }

    pub(super) fn comment(&mut self, name: &str, text: &str) -> Result<(), CHeaderExportError> {
        self.line("/*")?;
        if !name.is_empty() {
            self.comment_text(name)?;
        }
        self.comment_text(text)?;
        self.line(" */")
    }

    fn comment_text(&mut self, text: &str) -> Result<(), CHeaderExportError> {
        for line in text.lines() {
            // Keep C preprocessing, comment delimiters and line splicing inert.
            let line = line
                .replace('\\', "\\ ")
                .replace('?', "? ")
                .replace("/*", "/ *")
                .replace("*/", "* /");
            self.line(format_args!(" * {line}"))?;
        }
        Ok(())
    }
}
