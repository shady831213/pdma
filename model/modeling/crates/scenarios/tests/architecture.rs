//! Public API ownership and normal dependency boundaries of the PDMA model.

use std::{any::type_name, collections::BTreeSet, error::Error, process::Command};

#[test]
fn ring_protocol_types_are_defined_by_pdma_contract() {
    // Given: the protocol types exposed to software and hardware through contract.
    let owners = [
        type_name::<pdma_model_contract::ring::RingError>(),
        type_name::<pdma_model_contract::ring::RingPointers>(),
    ];
    // When: their defining Rust crate is inspected, including through reexports.
    // Then: ownership is PDMA-specific, not hidden in a framework facade.
    for owner in owners {
        assert!(
            owner.starts_with("pdma_model_contract::"),
            "wrong protocol owner: {owner}"
        );
    }
}

#[test]
fn frameworks_do_not_expose_pdma_ring_apis() {
    // Given: downstream programs trying to import PDMA ring APIs from frameworks.
    let cases = trybuild::TestCases::new();
    // When: the real compiler resolves those public API paths.
    // Then: both misplaced API surfaces are unavailable.
    cases.compile_fail("tests/architecture/schema_ring.rs");
    cases.compile_fail("tests/architecture/software_ring.rs");
}

#[test]
fn framework_normal_dependencies_remain_model_independent() -> Result<(), Box<dyn Error>> {
    // Given: each reusable building block with all of its supported features.
    for package in [
        "hw-model-schema",
        "hw-model-schema-macros",
        "hw-model-swenv",
        "hw-model-pipeline",
        "hw-model-io",
        "hw-model-irq",
    ] {
        // When: Cargo resolves its actual transitive normal dependencies.
        let dependencies = dependencies(package)?;
        // Then: no PDMA hardware, software or protocol package appears below it.
        assert!(
            dependencies
                .iter()
                .all(|dependency| !dependency.starts_with("pdma-model-")),
            "{package}: {dependencies:?}"
        );
    }
    Ok(())
}

#[test]
fn shared_software_depends_on_contract_not_host_firmware_or_dma() -> Result<(), Box<dyn Error>> {
    // Given: the common PDMA software implementation used by both software domains.
    // When: Cargo resolves its normal dependency graph.
    let dependencies = dependencies("pdma-model-software")?;
    // Then: it uses contract and software execution without either consumer or hardware.
    assert!(dependencies.contains("pdma-model-contract"));
    assert!(dependencies.contains("hw-model-swenv"));
    for forbidden in [
        "pdma-model-host",
        "pdma-model-firmware",
        "pdma-model-dma",
        "pdma-model-hw-components",
    ] {
        assert!(
            !dependencies.contains(forbidden),
            "software depends on {forbidden}"
        );
    }
    for consumer in ["pdma-model-host", "pdma-model-firmware"] {
        assert!(self::dependencies(consumer)?.contains("pdma-model-software"));
    }
    Ok(())
}

#[test]
fn standalone_capabilities_do_not_require_unrelated_runtime_policy() -> Result<(), Box<dyn Error>> {
    // Given: each independent capability with optional integrations disabled.
    for (package, required, forbidden) in [
        (
            "hw-model-irq",
            &[][..],
            &["tokio", "hw-model-io", "hw-model-swenv"][..],
        ),
        (
            "hw-model-io",
            &[][..],
            &["tokio", "hw-model-schema", "hw-model-swenv"][..],
        ),
        (
            "hw-model-swenv",
            &["hw-model-io", "hw-model-irq", "tokio"][..],
            &["hw-model-schema"][..],
        ),
    ] {
        // When: Cargo resolves normal dependencies without workspace feature unification.
        let resolved = dependency_tree(package, "--no-default-features")?;
        // Software execution consumes IO/IRQ; the IO package itself remains std-only.
        for dependency in required {
            assert!(
                resolved.contains(*dependency),
                "{package} omits {dependency}"
            );
        }
        // Then: selecting one capability does not import unrelated model or execution policy.
        for dependency in forbidden {
            assert!(
                !resolved.contains(*dependency),
                "{package} depends on {dependency}"
            );
        }
    }
    Ok(())
}

fn dependencies(package: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    dependency_tree(package, "--all-features")
}

fn dependency_tree(package: &str, features: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let output = Command::new(env!("CARGO"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "tree",
            "--offline",
            "--package",
            package,
            "--edges",
            "normal",
            "--prefix",
            "none",
            features,
        ])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(String::from_utf8(output.stderr)?).into());
    }
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect())
}
