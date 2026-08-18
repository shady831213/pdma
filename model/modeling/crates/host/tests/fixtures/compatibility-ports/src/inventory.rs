use std::fs;

#[allow(dead_code)]
mod source;
#[allow(dead_code)]
mod symbols;
#[cfg(test)]
mod tests;

const HEADER: &str = "owner\tpath\tline\tsymbol\treplacement\tretention\ttask26_removal";

pub fn render() -> Option<String> {
    Some(format!("{HEADER}\n"))
}

pub fn is_complete(manifest: &str) -> bool {
    manifest == format!("{HEADER}\n")
}

pub fn new_alias_count() -> usize {
    let Some(root) = source::workspace_root() else {
        return 1;
    };
    [
        root.join("crates/host/src/compat.rs"),
        root.join("crates/host/src/compat"),
    ]
    .into_iter()
    .flat_map(|path| source::rust_files(&path))
    .filter_map(|path| fs::read_to_string(path).ok())
    .flat_map(|text| text.lines().map(str::to_owned).collect::<Vec<_>>())
    .filter(|line| line.trim_start().starts_with("pub type ") || line.contains(" as "))
    .count()
}

pub fn transition_evidence_complete() -> bool {
    let Some(modeling) = source::workspace_root() else {
        return false;
    };
    let Some(repository) = modeling.ancestors().nth(2) else {
        return false;
    };
    let evidence = repository.join(".omo/evidence/hardware-interface-schema-runtime-refactor");
    [
        (
            evidence.join("task-15/executor/transitional-export-manifest.tsv"),
            "task26_removal_criterion",
        ),
        (
            evidence.join("task-21/executor/task26-transitional-inventory.md"),
            "Root `AsyncDevicePort`",
        ),
        (
            evidence.join("task-22/executor/DoneClaim.md"),
            "PdmaRegisterPort",
        ),
        (
            evidence.join("task-23/executor/task26-reexport-ledger-delta.md"),
            "Retained unchanged for Task 26",
        ),
    ]
    .iter()
    .all(|(path, marker)| fs::read_to_string(path).is_ok_and(|text| text.contains(marker)))
}
