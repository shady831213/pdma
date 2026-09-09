use std::fmt::Write as _;

use crate::Documentation;

pub(super) fn push_documentation(output: &mut String, name: &str, docs: Documentation) {
    if docs == Documentation::EMPTY {
        return;
    }
    output.push_str("/*\n");
    for text in [
        Some(name).filter(|name| !name.is_empty()),
        docs.short_description,
        docs.description,
    ]
    .into_iter()
    .flatten()
    {
        for line in text.lines() {
            // Keep C preprocessing, comment delimiters and line splicing inert.
            let line = line
                .replace('\\', "\\ ")
                .replace('?', "? ")
                .replace("/*", "/ *")
                .replace("*/", "* /");
            let _ = writeln!(output, " * {line}");
        }
    }
    output.push_str(" */\n");
}
