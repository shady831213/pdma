//! Standalone role headers enforce static visibility and permission metadata.
#![cfg(feature = "export-c-header")]

#[test]
fn directory_export_produces_separate_role_headers() {
    // Given: the public generator and an unused output directory.
    let directory = std::env::temp_dir().join(format!("pdma-role-red-{}", std::process::id()));
    // When: requesting coherent role headers.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pdma-abi-header"))
        .args(["--channels", "3", "--write-dir"])
        .arg(&directory)
        .output()
        .unwrap();
    // Then: all three outputs exist and Host has no privileged window map.
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host = std::fs::read_to_string(directory.join("pdma_host.h")).unwrap();
    assert!(!host.contains("WINDOWCONTROL"));
    assert!(directory.join("pdma_common.h").is_file());
    assert!(directory.join("pdma_firmware.h").is_file());
    for role in ["host", "firmware"] {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/header_c/{role}.c"));
        let binary = directory.join(role);
        let compile = std::process::Command::new("cc")
            .args([
                "-std=c11",
                "-pedantic-errors",
                "-Wall",
                "-Wextra",
                "-Werror",
            ])
            .arg("-I")
            .arg(&directory)
            .arg(fixture)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        assert!(
            std::process::Command::new(binary)
                .status()
                .unwrap()
                .success()
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
