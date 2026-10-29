//! Tests the public layout export command and its checked-in RTL artifact.
use std::{error::Error, fs, path::PathBuf, process::Command};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn export_preserves_wire_offsets_masks_and_leaf_permissions() -> TestResult {
    // Given/When: a consumer requests the generated layout through its public CLI.
    let result = Command::new(env!("CARGO_BIN_EXE_pdma-rtl-layout")).output()?;
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout)?;
    // Then: independent wire-layout anchors distinguish width, write access and bit masks.
    for expected in [
        "const COMMAND_BASE_OFFSET: u32 = 0;",
        "const COMMAND_BASE_BYTES: u32 = 8;",
        "const COMMAND_PRODUCER_OFFSET: u32 = 12;",
        "const COMMAND_CONSUMER_WRITE: bbool = false;",
        "const EVENT_PRODUCER_WRITE: bbool = false;",
        "const EVENT_CONSUMER_WRITE: bbool = true;",
        "const CONTROL_RECOVERY_RESUME_OFFSET: u32 = 60;",
        "const CONTROL_RECOVERY_RESUME_READ: bbool = false;",
        "const CONTROL_IRQ_MASK_WRITABLE: u64 = 64'h0000000000000003;",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    let checked_in =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../rtl/src/dma/layout.veryl");
    assert_eq!(text, fs::read_to_string(checked_in)?);
    Ok(())
}

#[test]
fn check_rejects_stale_output_without_overwriting_it() -> TestResult {
    // Given: a unique temporary file containing intentionally stale output.
    let path = std::env::temp_dir().join(format!("pdma-rtl-stale-{}.veryl", std::process::id()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let cleanup = TemporaryFile(path);
    fs::write(&cleanup.0, b"stale\n")?;
    // When: check mode inspects the stale consumer artifact.
    let result = Command::new(env!("CARGO_BIN_EXE_pdma-rtl-layout"))
        .arg("--check")
        .arg(&cleanup.0)
        .output()?;
    // Then: it fails without replacing the artifact.
    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)?.contains("is stale"));
    assert_eq!(fs::read(&cleanup.0)?, b"stale\n");
    Ok(())
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0) {
            eprintln!("temporary layout cleanup failed: {error}");
        }
    }
}
