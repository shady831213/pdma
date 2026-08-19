use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_COMPILE: AtomicU64 = AtomicU64::new(0);

struct CompileSurfaceObservables {
    manual_irq_methods: usize,
    device_types: usize,
}

struct CompileDirectory(PathBuf);

impl CompileDirectory {
    fn create() -> Result<Self, String> {
        let sequence = NEXT_COMPILE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "pdma-host-core-compile-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).map_err(|error| format!("create {}: {error}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for CompileDirectory {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.0);
    }
}

fn verify_downstream_compile_surface() -> Result<CompileSurfaceObservables, String> {
    let current_exe = env::current_exe().map_err(|error| error.to_string())?;
    let deps = current_exe
        .parent()
        .ok_or_else(|| String::from("test executable has no dependency directory"))?;
    let host = find_rlib(deps, "libpdma_model_host-")?;
    let contract = find_rlib(deps, "libpdma_model_contract-")?;
    let runtime = find_rlib(deps, "libhardware_interface_runtime-")?;
    let output = CompileDirectory::create()?;
    let pass = compile_fixture(
        deps,
        &output.0,
        "pass_driver_port",
        "tests/host_core_surface/compile/pass_driver_port.rs",
        [&host, &contract, &runtime],
    )?;
    let fail = compile_fixture(
        deps,
        &output.0,
        "manual_driver_irq",
        "tests/host_core_surface/compile/manual_driver_irq.rs",
        [&host, &contract, &runtime],
    )?;
    let device_types = usize::from(!pass.status.success());
    let manual_irq_methods = usize::from(fail.status.success());
    if device_types != 0 {
        return Err(format!(
            "contract/runtime/host fake failed: {}",
            String::from_utf8_lossy(&pass.stderr)
        ));
    }
    let failure = String::from_utf8_lossy(&fail.stderr);
    if manual_irq_methods != 0 || !failure.contains("no method named `service_irq_on`") {
        return Err(format!(
            "ManualDriver compile boundary was not exact: {failure}"
        ));
    }
    Ok(CompileSurfaceObservables {
        manual_irq_methods,
        device_types,
    })
}

fn find_rlib(deps: &Path, prefix: &str) -> Result<PathBuf, String> {
    fs::read_dir(deps)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "rlib")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(prefix))
        })
        .max_by_key(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .ok_or_else(|| format!("missing {prefix}*.rlib in {}", deps.display()))
}

fn compile_fixture(
    deps: &Path,
    output: &Path,
    crate_name: &str,
    source: &str,
    libraries: [&Path; 3],
) -> Result<Output, String> {
    Command::new("rustc")
        .args([source, "--edition=2024", "--crate-name", crate_name])
        .arg("--out-dir")
        .arg(output)
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("pdma_model_host={}", libraries[0].display()))
        .arg("--extern")
        .arg(format!("pdma_model_contract={}", libraries[1].display()))
        .arg("--extern")
        .arg(format!(
            "hardware_interface_runtime={}",
            libraries[2].display()
        ))
        .output()
        .map_err(|error| error.to_string())
}
