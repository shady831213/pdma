use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_COMPILE: AtomicU64 = AtomicU64::new(0);

#[expect(
    clippy::redundant_pub_crate,
    reason = "the compile fixture exposes observations only to its parent test module"
)]
pub(super) struct CompileSurfaceObservables {
    pub(super) manual_irq_methods: usize,
    pub(super) device_types: usize,
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

#[expect(
    clippy::redundant_pub_crate,
    reason = "the private compile helper is called only by its parent test module"
)]
pub(super) fn verify_downstream_compile_surface() -> Result<CompileSurfaceObservables, String> {
    let output = CompileDirectory::create()?;
    create_compile_crate(&output.0)?;
    let pass = compile_fixture(
        &output.0,
        "tests/host_core_surface/compile/pass_driver_port.rs",
    )?;
    let fail = compile_fixture(
        &output.0,
        "tests/host_core_surface/compile/manual_driver_irq.rs",
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

fn create_compile_crate(output: &Path) -> Result<(), String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| String::from("host crate has no workspace directory"))?;
    fs::create_dir(output.join("src"))
        .map_err(|error| format!("create compile fixture source: {error}"))?;
    let manifest = format!(
        "[package]\nname = \"pdma-host-compile-surface\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\npdma-model-contract = {{ path = \"{}\" }}\npdma-model-host = {{ path = \"{}\" }}\n\n[workspace]\n",
        workspace.join("crates/contract").display(),
        workspace.join("crates/host").display(),
    );
    fs::write(output.join("Cargo.toml"), manifest)
        .map_err(|error| format!("write compile fixture manifest: {error}"))
}

fn compile_fixture(output: &Path, source: &str) -> Result<Output, String> {
    fs::copy(source, output.join("src/main.rs"))
        .map_err(|error| format!("copy {source}: {error}"))?;
    Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline", "--manifest-path"])
        .arg(output.join("Cargo.toml"))
        .output()
        .map_err(|error| error.to_string())
}
