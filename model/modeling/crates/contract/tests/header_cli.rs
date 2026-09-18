//! Coherent header generation and non-mutating consistency checks.
#![cfg(feature = "export-c-header")]

use std::{
    error::Error,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
const HEADERS: [&str; 3] = ["pdma_common.h", "pdma_host.h", "pdma_firmware.h"];

#[test]
fn checked_in_headers_match_generator() -> Result<(), Box<dyn Error>> {
    // Given: the checked-in public role header set.
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../include");
    // When: consistency is checked against the current schemas.
    let output = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .args(["--channels", "3", "--check-dir"])
        .arg(directory)
        .output()?;
    // Then: every generated artifact matches.
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn write_is_deterministic_for_all_headers() -> Result<(), Box<dyn Error>> {
    // Given: two isolated output directories.
    let first = TempDirectory::new()?;
    let second = TempDirectory::new()?;
    // When: exporting the same hardware instance twice.
    for directory in [&first, &second] {
        generate(directory, 3)?;
    }
    // Then: all three artifacts are byte-identical.
    for name in HEADERS {
        assert_eq!(
            fs::read(first.0.join(name))?,
            fs::read(second.0.join(name))?
        );
    }
    Ok(())
}

#[test]
fn check_reports_each_artifacts_drift_without_modifying_files() -> Result<(), Box<dyn Error>> {
    // Given: each role artifact and the shared wire artifact.
    let temp = TempDirectory::new()?;
    for name in HEADERS {
        for contents in [Some(b"stale header".as_slice()), None] {
            generate(&temp, 3)?;
            let path = temp.0.join(name);
            match contents {
                Some(bytes) => fs::write(&path, bytes)?,
                None => fs::remove_file(&path)?,
            }
            let before = HEADERS.map(|file| fs::read(temp.0.join(file)).ok());
            // When: checking a stale or missing member of the set.
            let result = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
                .args(["--channels", "3", "--check-dir"])
                .arg(&temp.0)
                .output()?;
            // Then: failure preserves every existing artifact exactly.
            assert!(!result.status.success());
            assert_eq!(HEADERS.map(|file| fs::read(temp.0.join(file)).ok()), before);
        }
    }
    Ok(())
}

#[test]
fn invalid_arguments_fail_before_writing() -> Result<(), Box<dyn Error>> {
    // Given: a sentinel in the target directory.
    let temp = TempDirectory::new()?;
    fs::write(temp.0.join("pdma_host.h"), b"preserve me")?;
    for args in [
        vec!["--unknown"],
        vec!["--write-dir"],
        vec!["--check-dir"],
        vec!["--help", "extra"],
        vec!["--view", "unsupported"],
        vec!["--write"],
        vec!["--check"],
    ] {
        // When: invoking with an unsupported role, malformed or retired option.
        let output = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
            .args(["--channels", "3"])
            .args(args)
            .output()?;
        // Then: no target mutation occurs.
        assert!(!output.status.success());
        assert_eq!(fs::read(temp.0.join("pdma_host.h"))?, b"preserve me");
    }
    Ok(())
}

#[test]
fn help_describes_check_mode() -> Result<(), Box<dyn Error>> {
    // Given: the public generator executable.
    // When: requesting usage.
    let output = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .arg("--help")
        .output()?;
    // Then: help succeeds and identifies the directory-check option.
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)?.contains("--check-dir"));
    Ok(())
}

#[test]
fn channel_count_is_required_before_export() -> Result<(), Box<dyn Error>> {
    // Given: missing, malformed, empty and overflowing configurations.
    let temp = TempDirectory::new()?;
    for args in [
        vec![],
        vec!["--channels"],
        vec!["--channels", "zero"],
        vec!["--channels", "0"],
        vec!["--channels", "100000000"],
        vec!["--channels", "4294967296"],
        vec!["--channels", "18446744073709551615"],
    ] {
        // When: exporting the selected instance.
        let result = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
            .args(args)
            .arg("--write-dir")
            .arg(&temp.0)
            .output()?;
        // Then: invalid geometry cannot produce any header.
        assert!(!result.status.success());
        assert!(HEADERS.iter().all(|name| !temp.0.join(name).exists()));
    }
    Ok(())
}

#[test]
fn c_consumers_observe_each_selected_instance_extent() -> Result<(), Box<dyn Error>> {
    use std::{io::Write, process::Stdio};
    // Given: independent extents for three distinct instance counts.
    let temp = TempDirectory::new()?;
    for (count, extent) in [(1, 8192), (3, 16384), (5, 24576)] {
        generate(&temp, count)?;
        // When: both role headers compile together under strict C11.
        let mut compiler = Command::new("cc")
            .args([
                "-std=c11",
                "-pedantic-errors",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-x",
                "c",
                "-",
                "-fsyntax-only",
            ])
            .arg("-I")
            .arg(&temp.0)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        write!(
            compiler.stdin.take().expect("compiler stdin"),
            "#include \"pdma_host.h\"\n#include \"pdma_firmware.h\"\n_Static_assert(C_HDMA_SIZE == {extent}, \"host extent\");\n_Static_assert(C_FDMA_SIZE == {extent}, \"firmware extent\");\n"
        )?;
        let output = compiler.wait_with_output()?;
        // Then: both roles retain the physical map extent.
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn generate(directory: &TempDirectory, count: usize) -> Result<(), Box<dyn Error>> {
    let result = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .args(["--channels", &count.to_string(), "--write-dir"])
        .arg(&directory.0)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

struct TempDirectory(PathBuf);
impl TempDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pdma-header-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("cleanup {}: {error}", self.0.display());
        }
    }
}
