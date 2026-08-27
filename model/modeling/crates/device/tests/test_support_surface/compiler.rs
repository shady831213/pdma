use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_COMPILE: AtomicU64 = AtomicU64::new(0);

struct CompileProbe {
    succeeded: bool,
    stderr: String,
}

impl CompileProbe {
    fn mentions_all<const N: usize>(&self, symbols: [&str; N]) -> bool {
        symbols
            .into_iter()
            .all(|symbol| self.stderr.contains(symbol))
    }
}

struct CompileSurface {
    final_owners: CompileProbe,
    stale_root_snapshots: CompileProbe,
    stale_root_runtime: CompileProbe,
}

struct CompileDirectory(PathBuf);

impl CompileDirectory {
    fn create() -> Result<Self, String> {
        let sequence = NEXT_COMPILE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "pdma-device-test-support-compile-{}-{sequence}",
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

fn compile_downstream_surface() -> Result<CompileSurface, String> {
    let output = CompileDirectory::create()?;
    create_compile_crate(&output.0)?;
    Ok(CompileSurface {
        final_owners: compile_fixture(&output.0, "final_owners")?,
        stale_root_snapshots: compile_fixture(&output.0, "stale_root_snapshots")?,
        stale_root_runtime: compile_fixture(&output.0, "stale_root_runtime")?,
    })
}

fn create_compile_crate(output: &Path) -> Result<(), String> {
    let source = output.join("src/bin");
    fs::create_dir_all(&source).map_err(|error| format!("create fixture source: {error}"))?;
    for fixture in [
        "final_owners.rs",
        "stale_root_snapshots.rs",
        "stale_root_runtime.rs",
    ] {
        fs::copy(fixture_source(fixture), source.join(fixture))
            .map_err(|error| format!("copy {fixture}: {error}"))?;
    }
    write_manifest(output)
}

fn compile_fixture(output: &Path, bin: &str) -> Result<CompileProbe, String> {
    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline", "--manifest-path"])
        .arg(output.join("Cargo.toml"))
        .args(["--bin", bin])
        .env("CARGO_TARGET_DIR", output.join("target"))
        .output()
        .map_err(|error| format!("run {bin}: {error}"))?;
    Ok(CompileProbe {
        succeeded: output.status.success(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn fixture_source(source: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_support_surface/compile")
        .join(source)
}

fn write_manifest(output: &Path) -> Result<(), String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| String::from("device crate has no workspace directory"))?;
    let manifest = format!(
        "[package]\nname = \"pdma-device-test-support-compile\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nhardware-interface-runtime = {{ path = \"{}\" }}\npdma-model-device = {{ path = \"{}\" }}\n\n[workspace]\n",
        workspace
            .join("crates/hardware-interface-runtime")
            .display(),
        workspace.join("crates/device").display(),
    );
    fs::write(output.join("Cargo.toml"), manifest)
        .map_err(|error| format!("write compile fixture manifest: {error}"))
}
