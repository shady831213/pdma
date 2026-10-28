#![allow(missing_docs, reason = "external fixture test")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Case {
    name: &'static str,
    feature: &'static str,
    fragments: &'static [&'static str],
}

const CASES: [Case; 3] = [
    Case {
        name: "escape_reference",
        feature: "negative-escape-reference",
        fragments: &["error[E0308]", "for<'value> FnOnce"],
    },
    Case {
        name: "async_storage_closure",
        feature: "negative-async-storage-closure",
        fragments: &["error[E0308]", "async closure body", "FnOnce"],
    },
    Case {
        name: "dyn_stage",
        feature: "negative-dyn-stage",
        fragments: &["error[E0038]", "not dyn compatible", "Sized"],
    },
];

fn compile(case: &Case, target: &Path) -> Result<Output, Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    Ok(Command::new("cargo")
        .args([
            "check",
            "--quiet",
            "--manifest-path",
            manifest.to_str().ok_or("manifest is not UTF-8")?,
            "--features",
            case.feature,
            "--bin",
            case.name,
        ])
        .env("CARGO_TARGET_DIR", target)
        .output()?)
}

#[test]
fn invalid_boundary_consumers_fail_for_structural_type_reasons()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: three consumers that cross distinct public type/lifetime boundaries.
    let target =
        std::env::temp_dir().join(format!("task19-compile-negative-{}", std::process::id()));
    fs::create_dir_all(&target)?;

    // When: each consumer is checked as an independent downstream binary.
    for case in &CASES {
        let output = compile(case, &target)?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Then: compilation fails and every case-specific structural fragment is present.
        if output.status.success() || !case.fragments.iter().all(|item| stderr.contains(item)) {
            fs::remove_dir_all(&target)?;
            return Err(format!(
                "{} diagnostic mismatch; status={}; stderr={stderr}",
                case.name, output.status
            )
            .into());
        }
        println!(
            "compile_negative case={} rejected=true reasons={}",
            case.name,
            case.fragments.join("+")
        );
    }
    fs::remove_dir_all(target)?;
    Ok(())
}
