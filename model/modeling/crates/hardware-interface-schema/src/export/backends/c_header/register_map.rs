use std::fmt::Write as _;

mod names;

use crate::{
    export::{Artifact, ArtifactSink, Exporter},
    register::{Access, ResetKind},
    register_map::ResolvedRegisterMap,
};

use super::{
    C_MEDIA_TYPE, CHeaderExportError, FragmentConfig,
    documentation::push_documentation,
    model::{RegisterCHeaderBank, RegisterCHeaderModel, RegisterMapCHeaderModel},
};
use names::{BankNames, map_names};

/// Renders one typed register map as a generic C11 fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterMapCHeaderExporter {
    config: FragmentConfig,
}

impl RegisterMapCHeaderExporter {
    /// Creates a renderer with caller-owned artifact and C symbol names.
    #[must_use]
    pub fn new(logical_name: impl Into<String>, symbol_prefix: impl AsRef<str>) -> Self {
        Self {
            config: FragmentConfig::new(logical_name, symbol_prefix),
        }
    }
}

impl<I: Copy + Eq + 'static, F: Copy + Eq + core::fmt::Debug> Exporter<ResolvedRegisterMap<I, F>>
    for RegisterMapCHeaderExporter
{
    type Error = CHeaderExportError;

    fn export<Sink>(
        &self,
        schema: &ResolvedRegisterMap<I, F>,
        sink: &mut Sink,
    ) -> Result<(), Self::Error>
    where
        Sink: ArtifactSink<Error = Self::Error>,
    {
        let model = RegisterMapCHeaderModel::from_resolved(schema);
        let paths_and_names = schema
            .registers()
            .iter()
            .map(|register| {
                (
                    register.path().into(),
                    super::naming::c_identifier(register.path()),
                )
            })
            .collect::<Vec<_>>();
        super::naming::reject_collisions(&paths_and_names)?;
        sink.emit(Artifact {
            logical_name: self.config.logical_name.clone(),
            media_type: String::from(C_MEDIA_TYPE),
            contents: render(&model, &self.config.symbol_prefix).into_bytes(),
        })
    }
}

fn render<I: Copy + Eq + 'static, F: Copy + Eq + core::fmt::Debug>(
    model: &RegisterMapCHeaderModel<I, F>,
    prefix: &str,
) -> String {
    let mut output = String::new();
    push_documentation(&mut output, "", model.documentation());
    push_map_constants(&mut output, prefix, model.size());
    for (path, documentation) in model.groups() {
        push_documentation(&mut output, path, *documentation);
    }
    let names = map_names(model);
    for ((register, name), offset) in model
        .fixed()
        .iter()
        .copied()
        .zip(&names.fixed)
        .zip(&names.fixed_offsets)
    {
        output.push('\n');
        let name = format!("{prefix}_{name}");
        let offset = format!("{prefix}_{offset}");
        push_register(
            &mut output,
            RegisterNames {
                prefix,
                name: &name,
                offset: Some((&offset, register.offset())),
            },
            register,
        );
    }
    for (bank, names) in model.banks().iter().zip(&names.banks) {
        output.push('\n');
        push_bank(&mut output, prefix, bank, names);
    }
    output
}

fn push_map_constants(output: &mut String, prefix: &str, size: usize) {
    let _ = writeln!(output, "#define {prefix}_SIZE UINT64_C({size})");
    let _ = writeln!(output, "#define {prefix}_ACCESS_READ_ONLY UINT32_C(0)");
    let _ = writeln!(output, "#define {prefix}_ACCESS_READ_WRITE UINT32_C(1)");
    let _ = writeln!(output, "#define {prefix}_ACCESS_WRITE_ONLY UINT32_C(2)");
    let _ = writeln!(output, "#define {prefix}_RESET_CONSTANT UINT32_C(0)");
    let _ = writeln!(output, "#define {prefix}_RESET_ZERO UINT32_C(1)");
    let _ = writeln!(output, "#define {prefix}_RESET_POST_RESET UINT32_C(2)");
}

fn push_register<I: Copy, F: Copy>(
    output: &mut String,
    names: RegisterNames<'_>,
    register: RegisterCHeaderModel<I, F>,
) {
    let RegisterNames {
        prefix,
        name,
        offset,
    } = names;
    push_documentation(output, "", register.documentation());
    for field in register.fields() {
        push_documentation(output, field.name(), field.documentation());
    }
    if let Some((offset_name, offset)) = offset {
        let _ = writeln!(output, "#define {offset_name} UINT64_C({offset})");
    }
    let _ = writeln!(
        output,
        "#define {name}_WIDTH UINT64_C({})",
        register.width()
    );
    let access = match register.access() {
        Access::ReadOnly => "READ_ONLY",
        Access::ReadWrite => "READ_WRITE",
        Access::WriteOnly => "WRITE_ONLY",
    };
    let reset = match register.reset_kind() {
        ResetKind::Constant => "CONSTANT",
        ResetKind::Zero => "ZERO",
        ResetKind::PostReset => "POST_RESET",
    };
    let _ = writeln!(output, "#define {name}_ACCESS {prefix}_ACCESS_{access}");
    let _ = writeln!(output, "#define {name}_RESET_KIND {prefix}_RESET_{reset}");
    let _ = writeln!(
        output,
        "#define {name}_RESET_VALUE UINT64_C({})",
        register.reset_value()
    );
    let _ = writeln!(
        output,
        "#define {name}_WRITABLE_MASK UINT64_C({})",
        register.writable_mask()
    );
}

fn push_bank<I: Copy, F: Copy>(
    output: &mut String,
    prefix: &str,
    bank: &RegisterCHeaderBank<I, F>,
    names: &BankNames,
) {
    let bank_name = format!("{prefix}_{}", names.name);
    push_documentation(output, "", bank.documentation());
    let _ = writeln!(output, "#define {bank_name}_BASE UINT64_C({})", bank.base());
    let _ = writeln!(
        output,
        "#define {bank_name}_COUNT UINT64_C({})",
        bank.count()
    );
    let _ = writeln!(
        output,
        "#define {bank_name}_STRIDE UINT64_C({})",
        bank.stride()
    );
    for (register, member) in bank.registers().iter().copied().zip(&names.members) {
        let name = format!("{bank_name}_{member}");
        push_register(
            output,
            RegisterNames {
                prefix,
                name: &name,
                offset: None,
            },
            register,
        );
    }
    for (instance, offsets) in (0..bank.count()).zip(&names.offsets) {
        for ((register, _), offset_name) in bank
            .registers()
            .iter()
            .copied()
            .zip(&names.members)
            .zip(offsets)
        {
            let offset = bank.base() + instance * bank.stride() + register.offset();
            let _ = writeln!(output, "#define {prefix}_{offset_name} UINT64_C({offset})");
        }
    }
}

#[derive(Clone, Copy)]
struct RegisterNames<'a> {
    prefix: &'a str,
    name: &'a str,
    offset: Option<(&'a str, usize)>,
}
