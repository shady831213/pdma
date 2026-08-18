use super::symbols::SYMBOLS;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub fn workspace_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .map(Path::to_path_buf)
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            if path.ends_with("fixtures") {
                continue;
            }
            if !collect_files(&path, files) {
                return false;
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    true
}

pub fn rust_files(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if path.is_dir() {
        let _ = collect_files(path, &mut files);
    } else {
        files.push(path.to_path_buf());
    }
    files
}

fn owner(path: &str) -> &'static str {
    if path.starts_with("crates/scenarios/tests/message_irq_linear_copy") {
        "task26-canonical-scenario"
    } else if path.starts_with("crates/scenarios/tests") {
        "task25-scenario"
    } else if path.starts_with("crates/host/tests") {
        "task26-host-regression"
    } else if path.starts_with("crates/host/src") {
        "task26-host-compat"
    } else {
        "task26-device-compat"
    }
}

fn retention(path: &str) -> &'static str {
    match owner(path) {
        "task25-scenario" => "scenario migration is owned by Task25",
        "task26-canonical-scenario" => "exact canonical migration is owned by Task26",
        "task26-host-regression" => "legacy regression remains until replacement migration",
        "task26-host-compat" => "compat implementation remains until consumers migrate",
        _ => "device root re-export remains until consumers migrate",
    }
}

fn removal(path: &str) -> &'static str {
    if owner(path) == "task25-scenario" {
        "Task25 replacement is green and Task26 removes the old root"
    } else {
        "replacement fixture is green and Task26 removes the old root"
    }
}

fn old_host_source(path: &str) -> bool {
    [
        "crates/host/src/lib.rs",
        "crates/host/src/adapters.rs",
        "crates/host/src/interrupt.rs",
        "crates/host/src/bus",
        "crates/host/src/context",
        "crates/host/src/irq_delivery",
        "crates/host/src/port",
        "crates/host/src/register",
    ]
    .iter()
    .any(|prefix| {
        path == *prefix || path == format!("{prefix}.rs") || path.starts_with(&format!("{prefix}/"))
    })
}

fn imports_root(text: &str, symbol: &str) -> bool {
    text.split(';').any(|statement| {
        let compact = statement.split_whitespace().collect::<String>();
        let host_root = compact.contains("usepdma_model_host::{")
            || compact.contains(&format!("usepdma_model_host::{symbol}"))
            || (compact.contains("usepdma_model_host::interrupt::{") && compact.contains(symbol));
        let device_root = symbol == "SyncDevice"
            && (compact.contains("usepdma_model_device::{")
                || compact.contains("usepdma_model_device::SyncDevice"));
        host_root || device_root
    })
}

fn live_site(path: &str, text: &str, line: &str, symbol: &str) -> bool {
    if old_host_source(path)
        || path == "crates/device/src/lib.rs"
        || path == "crates/device/src/runner.rs"
    {
        return true;
    }
    imports_root(text, symbol)
        || line.contains(&format!("pdma_model_host::{symbol}"))
        || (symbol == "SyncDevice" && line.contains("pdma_model_device::SyncDevice"))
}

pub fn live_rows() -> Option<BTreeSet<String>> {
    let root = workspace_root()?;
    let mut files = Vec::new();
    for relative in [
        "crates/host/src",
        "crates/host/tests",
        "crates/scenarios/tests",
    ] {
        if !collect_files(&root.join(relative), &mut files) {
            return None;
        }
    }
    files.extend([
        root.join("crates/device/src/lib.rs"),
        root.join("crates/device/src/runner.rs"),
    ]);
    files.sort();
    let mut rows = BTreeSet::new();
    for file in files {
        let text = fs::read_to_string(&file).ok()?;
        let relative = file.strip_prefix(&root).ok()?.to_string_lossy();
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for (symbol, replacement) in SYMBOLS {
                let mentions = line
                    .split(|character: char| !character.is_alphanumeric() && character != '_')
                    .any(|word| word == *symbol);
                if mentions && live_site(&relative, &text, line, symbol) {
                    rows.insert(format!(
                        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        owner(&relative),
                        relative,
                        index + 1,
                        symbol,
                        replacement,
                        retention(&relative),
                        removal(&relative)
                    ));
                }
            }
        }
    }
    Some(rows)
}
