use std::fmt::Write as _;

use crate::EnumDefinition;

use super::{documentation::push_documentation, naming::c_identifier};

pub(super) fn push_enum(output: &mut String, prefix: &str, definition: EnumDefinition) {
    for variant in definition.variants() {
        push_documentation(output, variant.name(), variant.documentation());
        let name = c_identifier(variant.name());
        let _ = writeln!(
            output,
            "#define {prefix}_{name} UINT64_C({})",
            variant.value()
        );
    }
}
