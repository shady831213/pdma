//! Header generation and non-mutating consistency checks.

#![cfg(feature = "export-c-header")]

use std::{
    error::Error,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn checked_in_header_matches_generator() -> Result<(), Box<dyn Error>> {
    // Given: the checked-in public header.
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../include/pdma_model_abi.h");
    // When: consistency is checked against the current schemas.
    let output = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .args(["--channels", "3"])
        .arg("--check")
        .arg(header)
        .output()?;
    // Then: no regeneration is needed.
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn write_matches_stdout() -> Result<(), Box<dyn Error>> {
    // Given: an isolated target and the stdout representation.
    let temp = TempDirectory::new()?;
    let path = temp.0.join("header.h");
    let binary = env!("CARGO_BIN_EXE_pdma-abi-header");
    let generated = Command::new(binary).args(["--channels", "3"]).output()?;
    assert!(generated.status.success());
    // When: writing the generated header to a file.
    let written = Command::new(binary)
        .args(["--channels", "3"])
        .arg("--write")
        .arg(&path)
        .output()?;
    // Then: write emits exactly the stdout header.
    assert!(written.status.success());
    assert_eq!(fs::read(&path)?, generated.stdout);
    Ok(())
}

#[test]
fn check_reports_drift_without_modifying_files() -> Result<(), Box<dyn Error>> {
    // Given: current, stale and absent header targets.
    let temp = TempDirectory::new()?;
    let binary = env!("CARGO_BIN_EXE_pdma-abi-header");
    let generated = Command::new(binary).args(["--channels", "3"]).output()?;
    assert!(generated.status.success());
    for (name, contents, success) in [
        ("current.h", Some(generated.stdout.as_slice()), true),
        ("stale.h", Some(b"stale header".as_slice()), false),
        ("missing.h", None, false),
    ] {
        let path = temp.0.join(name);
        if let Some(bytes) = contents {
            fs::write(&path, bytes)?;
        }
        // When: checking each target.
        let output = Command::new(binary)
            .args(["--channels", "3"])
            .arg("--check")
            .arg(&path)
            .output()?;
        // Then: only matching bytes pass and the target is unchanged.
        assert_eq!(output.status.success(), success);
        assert_eq!(fs::read(&path).ok().as_deref(), contents);
    }
    Ok(())
}

#[test]
fn invalid_arguments_fail_before_writing() -> Result<(), Box<dyn Error>> {
    // Given: a file that invalid invocations must preserve.
    let temp = TempDirectory::new()?;
    let path = temp.0.join("header.h");
    fs::write(&path, b"preserve me")?;
    let binary = env!("CARGO_BIN_EXE_pdma-abi-header");
    for args in [
        vec![std::ffi::OsStr::new("--unknown")],
        vec![std::ffi::OsStr::new("--write")],
        vec![std::ffi::OsStr::new("--check")],
        vec![std::ffi::OsStr::new("--help"), "extra".as_ref()],
        vec!["--write".as_ref(), path.as_os_str(), "extra".as_ref()],
        vec!["--check".as_ref(), path.as_os_str(), "extra".as_ref()],
    ] {
        // When: invoking with malformed or excess arguments.
        // Then: the command fails before touching the target.
        assert!(
            !Command::new(binary)
                .args(["--channels", "3"])
                .args(args)
                .output()?
                .status
                .success()
        );
        assert_eq!(fs::read(&path)?, b"preserve me");
    }
    Ok(())
}

#[test]
fn help_describes_check_mode() -> Result<(), Box<dyn Error>> {
    // Given: the public generator executable.
    let binary = env!("CARGO_BIN_EXE_pdma-abi-header");
    // When: requesting usage.
    let help = Command::new(binary).arg("--help").output()?;
    // Then: help succeeds and lists consistency checking.
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)?.contains("--check"));
    Ok(())
}

#[test]
fn channel_count_is_required_before_export() -> Result<(), Box<dyn Error>> {
    // Given: missing, malformed, empty and overflowing instance configurations.
    let binary = env!("CARGO_BIN_EXE_pdma-abi-header");
    for args in [
        vec![],
        vec!["--channels"],
        vec!["--channels", "zero"],
        vec!["--channels", "0"],
        vec!["--channels", "18446744073709551615"],
    ] {
        // When: exporting the selected instance.
        let output = Command::new(binary).args(args).output()?;
        // Then: no header is emitted for invalid instance geometry.
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    Ok(())
}

#[test]
fn c_consumer_observes_each_selected_instance_extent() -> Result<(), Box<dyn Error>> {
    use std::io::Write;
    use std::process::Stdio;
    // Given: several instance channel counts and independently specified extents.
    let temp = TempDirectory::new()?;
    for (count, extent) in [(1, 84), (3, 228), (5, 372)] {
        let header = Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
            .args(["--channels", &count.to_string()])
            .output()?;
        assert!(header.status.success());
        fs::write(temp.0.join("pdma_model_abi.h"), header.stdout)?;
        // When: a strict C11 consumer compiles against that generated instance.
        let mut compile = Command::new("cc")
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
            compile.stdin.take().expect("compiler stdin"),
            "#include \"pdma_model_abi.h\"\n_Static_assert(C_PDMA_SIZE == {extent}, \"instance extent\");\n"
        )?;
        let result = compile.wait_with_output()?;
        // Then: the exported root extent matches the chosen hardware instance.
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
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
