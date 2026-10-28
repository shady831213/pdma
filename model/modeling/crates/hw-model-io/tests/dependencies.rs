//! Access interfaces, built-in memory and observation need only the standard library.

use std::{error::Error, process::Command};

#[test]
fn io_has_no_normal_package_dependencies() -> Result<(), Box<dyn Error>> {
    // Given: IO selected independently of its integration-test backends.
    let package = env!("CARGO_PKG_NAME");
    // When: Cargo resolves only normal dependencies, without workspace features.
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout)?;
    let packages: Vec<_> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    // Then: built-in memory adds no schema, executor or other package dependency.
    assert_eq!(packages, [package]);
    Ok(())
}
