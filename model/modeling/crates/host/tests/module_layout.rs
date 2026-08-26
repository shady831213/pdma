//! Host module-layout regression checks.

use std::path::Path;

#[test]
fn host_layers_follow_their_rust_module_paths() {
    // Given: the required semantic and exact-register host module layout.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let required = [
        "semantic/mod.rs",
        "semantic/driver/mod.rs",
        "semantic/port/mod.rs",
        "semantic/error.rs",
        "register/mod.rs",
        "register/bus.rs",
        "register/port/mod.rs",
    ];
    let retired = ["driver.rs", "port.rs", "core.rs", "register.rs", "bus.rs"];

    // When: physical paths are compared with the public ownership tree.
    let missing = required
        .iter()
        .filter(|path| !source.join(path).exists())
        .copied()
        .collect::<Vec<_>>();
    let stale = retired
        .iter()
        .filter(|path| source.join(path).exists())
        .copied()
        .collect::<Vec<_>>();

    // Then: every layer is conventional and no flattened owner remains.
    assert!(
        missing.is_empty(),
        "missing module-owned paths: {missing:?}"
    );
    assert!(
        stale.is_empty(),
        "stale flattened host paths remain: {stale:?}"
    );
}

#[test]
fn host_layer_facades_need_no_path_overrides() {
    // Given: each facade that owns a host layer or nested implementation.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let facades = [
        "semantic/mod.rs",
        "semantic/driver/mod.rs",
        "semantic/port/mod.rs",
        "register/mod.rs",
        "register/port/mod.rs",
    ];

    // When: their conventional module declarations are inspected.
    let overrides = facades
        .iter()
        .filter_map(|path| {
            std::fs::read_to_string(source.join(path))
                .ok()
                .filter(|contents| contents.contains("#[path"))
                .map(|_| *path)
        })
        .collect::<Vec<_>>();

    // Then: none requires a filesystem alias.
    assert!(
        overrides.is_empty(),
        "host facades still override module paths: {overrides:?}"
    );
}
