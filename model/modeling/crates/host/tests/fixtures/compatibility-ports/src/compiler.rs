use std::{fs, path::Path, process::Command};

fn library(deps: &Path, prefix: &str) -> Option<std::path::PathBuf> {
    fs::read_dir(deps)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "rlib")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(prefix))
        })
}

fn compile(source: &Path) -> Option<std::process::Output> {
    let executable = std::env::current_exe().ok()?;
    let deps = executable.parent()?.join("deps");
    let output = std::env::temp_dir().join(format!("pdma-task24-negative-{}", std::process::id()));
    fs::create_dir_all(&output).ok()?;
    let host = library(&deps, "libpdma_model_host-")?;
    let device = library(&deps, "libpdma_model_device-")?;
    let contract = library(&deps, "libpdma_model_contract-")?;
    let runtime = library(&deps, "libhardware_interface_runtime-")?;
    let result = Command::new("rustc")
        .arg(source)
        .args(["--edition=2024", "--crate-name", "task24_boundary"])
        .arg("--out-dir")
        .arg(&output)
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("pdma_model_host={}", host.display()))
        .arg("--extern")
        .arg(format!("pdma_model_device={}", device.display()))
        .arg("--extern")
        .arg(format!("pdma_model_contract={}", contract.display()))
        .arg("--extern")
        .arg(format!("hardware_interface_runtime={}", runtime.display()))
        .output()
        .ok();
    fs::remove_dir_all(output).ok()?;
    result
}

pub fn accepts(source: &Path) -> bool {
    compile(source).is_some_and(|output| output.status.success())
}

pub fn rejects(source: &Path, expected: &str) -> bool {
    compile(source).is_some_and(|output| {
        !output.status.success() && String::from_utf8_lossy(&output.stderr).contains(expected)
    })
}
