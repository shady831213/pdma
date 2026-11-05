use std::fmt::Display;
use std::io::Write;

use crate::Documentation;

use super::{CHeaderExportError, naming};

#[derive(Debug, Default)]
pub(super) struct Fragment {
    symbols: naming::Scope,
}

impl Fragment {
    pub(super) fn declare(&mut self, name: &str) -> Result<(), CHeaderExportError> {
        self.symbols.insert(name, name)
    }

    pub(super) fn define(
        &mut self,
        name: &str,
        value: impl Display,
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        self.declare(name)?;
        writeln!(writer, "#define {name} {value}").map_err(CHeaderExportError::Io)
    }

    pub(super) fn constants<const WIDTH: usize>(
        &mut self,
        prefix: &str,
        values: &[(&str, impl Display)],
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        for (name, value) in values {
            self.unsigned::<WIDTH>(&format!("{prefix}_{name}"), value, writer)?;
        }
        Ok(())
    }

    pub(super) fn unsigned<const WIDTH: usize>(
        &mut self,
        name: &str,
        value: impl Display,
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        self.define(name, format_args!("UINT{WIDTH}_C({value})"), writer)
    }

    pub(super) fn line(
        line: impl Display,
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        writeln!(writer, "{line}").map_err(CHeaderExportError::Io)
    }

    pub(super) fn documentation(
        name: &str,
        docs: Documentation,
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        if docs == Documentation::EMPTY {
            return Ok(());
        }
        writer.write_all(b"/*\n")?;
        if !name.is_empty() {
            Self::comment_text(name, writer)?;
        }
        for fragment in crate::export::value::documentation_fragments(docs) {
            let (crate::export::value::DocumentationFragment::ShortDescription(text)
            | crate::export::value::DocumentationFragment::Description(text)) = fragment;
            Self::comment_text(text, writer)?;
        }
        Self::line(" */", writer)
    }

    pub(super) fn comment(
        name: &str,
        text: &str,
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        Self::line("/*", writer)?;
        if !name.is_empty() {
            Self::comment_text(name, writer)?;
        }
        Self::comment_text(text, writer)?;
        Self::line(" */", writer)
    }

    fn comment_text(text: &str, writer: &mut dyn Write) -> Result<(), CHeaderExportError> {
        for line in text.lines() {
            // Keep C preprocessing, comment delimiters and line splicing inert.
            let line = line
                .replace('\\', "\\ ")
                .replace('?', "? ")
                .replace("/*", "/ *")
                .replace("*/", "* /");
            Self::line(format_args!(" * {line}"), writer)?;
        }
        Ok(())
    }
}
