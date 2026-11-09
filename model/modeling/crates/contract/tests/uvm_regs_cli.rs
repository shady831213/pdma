//! Register generator CLI consistency and stale-output rejection.
#![cfg(feature = "export-uvm-regs")]
use std::{fs, process::Command};

#[test]
fn write_check_stdout_and_stale_detection() {
    let executable = env!("CARGO_BIN_EXE_pdma-uvm-regs");
    let directory = std::env::temp_dir().join(format!("pdma-uvm-regs-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("register_model.sv");
    let stdout = Command::new(executable).output().unwrap();
    assert!(stdout.status.success());
    assert!(
        Command::new(executable)
            .arg("--write")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(fs::read(&path).unwrap(), stdout.stdout);
    assert!(
        Command::new(executable)
            .arg("--check")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    fs::write(&path, "stale").unwrap();
    let stale = Command::new(executable)
        .arg("--check")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert!(
        String::from_utf8(stale.stderr)
            .unwrap()
            .contains("is stale")
    );
    assert!(
        !Command::new(executable)
            .arg("--invalid")
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::remove_dir_all(directory).unwrap();
}
