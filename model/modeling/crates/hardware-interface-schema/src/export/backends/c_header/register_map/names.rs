use super::super::{
    model::RegisterMapCHeaderModel,
    naming::{c_identifier, disambiguate_identifiers},
};

pub(super) fn map_names<I: Copy + Eq + 'static, F: Copy + Eq + core::fmt::Debug>(
    model: &RegisterMapCHeaderModel<I, F>,
) -> MapNames {
    let bank_names = disambiguate_identifiers(
        model
            .banks()
            .iter()
            .map(|bank| c_identifier(bank.name()))
            .collect(),
    );
    let mut bases = model
        .fixed()
        .iter()
        .map(|register| c_identifier(register.name()))
        .collect::<Vec<_>>();
    for (bank, bank_name) in model.banks().iter().zip(&bank_names) {
        bases.extend(
            bank.registers()
                .iter()
                .map(|register| format!("{bank_name}_{}", c_identifier(register.name()))),
        );
    }
    let fixed_count = model.fixed().len();
    let mut bases = disambiguate_identifiers(bases).into_iter();
    let fixed: Vec<String> = bases.by_ref().take(fixed_count).collect();
    let mut banks = model
        .banks()
        .iter()
        .zip(bank_names)
        .map(|(bank, name)| {
            let bank_prefix = format!("{name}_");
            let members = bases
                .by_ref()
                .take(bank.registers().len())
                .map(|member| {
                    member
                        .strip_prefix(&bank_prefix)
                        .map_or_else(|| member.clone(), String::from)
                })
                .collect();
            BankNames {
                name,
                members,
                offsets: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    let mut offsets: Vec<String> = fixed.iter().map(|name| format!("{name}_OFFSET")).collect();
    for (bank, names) in model.banks().iter().zip(&banks) {
        for instance in 0..bank.count() {
            offsets.extend(
                names
                    .members
                    .iter()
                    .map(|member| format!("{}_{}_{}_OFFSET", names.name, instance, member)),
            );
        }
    }
    let mut offsets = disambiguate_identifiers(offsets).into_iter();
    let fixed_offsets = offsets.by_ref().take(fixed_count).collect();
    for (bank, names) in model.banks().iter().zip(&mut banks) {
        names.offsets = (0..bank.count())
            .map(|_| offsets.by_ref().take(bank.registers().len()).collect())
            .collect();
    }
    MapNames {
        fixed,
        fixed_offsets,
        banks,
    }
}

pub(super) struct MapNames {
    pub(super) fixed: Vec<String>,
    pub(super) fixed_offsets: Vec<String>,
    pub(super) banks: Vec<BankNames>,
}

pub(super) struct BankNames {
    pub(super) name: String,
    pub(super) members: Vec<String>,
    pub(super) offsets: Vec<Vec<String>>,
}
