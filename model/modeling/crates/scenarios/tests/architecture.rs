//! Public API ownership and normal dependency boundaries of the PDMA model.

use std::{any::type_name, collections::BTreeSet, error::Error, process::Command};

#[test]
fn ring_protocol_types_are_defined_by_pdma_contract() {
    // Given: the protocol types exposed to software and hardware through contract.
    let owners = [
        type_name::<pdma_model_contract::RingError>(),
        type_name::<pdma_model_contract::RingPointers>(),
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
    cases.compile_fail("tests/architecture/runtime_ring.rs");
}

#[test]
fn framework_normal_dependencies_remain_model_independent() -> Result<(), Box<dyn Error>> {
    // Given: each reusable building block with all of its supported features.
    for package in [
        "hardware-interface-schema",
        "hardware-interface-schema-macros",
        "hardware-interface-runtime",
        "hardware-interface-pipeline",
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
    // Then: it uses contract/runtime without depending on either consumer or hardware.
    assert!(dependencies.contains("pdma-model-contract"));
    assert!(dependencies.contains("hardware-interface-runtime"));
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

fn dependencies(package: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
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
            "--all-features",
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
