//! Device module-layout regression checks.

use std::path::Path;

#[test]
fn device_layers_follow_their_rust_module_paths() {
    // Given: explicit semantic, exact-ABI, and shared-execution owners.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let required = [
        "compat/mod.rs",
        "dma_space/mod.rs",
        "trace/mod.rs",
        "runner/mod.rs",
        "runner/semantic/mod.rs",
        "runner/semantic/memory.rs",
        "runner/semantic/queue.rs",
        "runner/abi/mod.rs",
        "runner/abi/bus/mod.rs",
        "runner/abi/mmio/mod.rs",
        "runner/abi/register_endpoint/mod.rs",
        "runner/execution/mod.rs",
        "runner/execution/command.rs",
        "runner/execution/io/frontend.rs",
    ];
    let retired = [
        "compat.rs",
        "dma_space.rs",
        "trace.rs",
        "runner.rs",
        "runner/backend.rs",
        "runner/native.rs",
        "runner/bus.rs",
        "runner/mmio.rs",
        "runner/register_endpoint.rs",
        "runner/memory.rs",
        "runner/queue_api.rs",
    ];

    // When: the physical tree is compared with device-layer ownership.
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

    // Then: each layer is directly navigable and stale facades are gone.
    assert!(
        missing.is_empty(),
        "missing module-owned paths: {missing:?}"
    );
    assert!(stale.is_empty(), "stale device paths remain: {stale:?}");
}

#[test]
fn device_layer_facades_need_no_path_overrides() {
    // Given: every facade introduced by the device ownership split.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let facades = [
        "runner/mod.rs",
        "runner/semantic/mod.rs",
        "runner/abi/mod.rs",
        "runner/abi/bus/mod.rs",
        "runner/abi/mmio/mod.rs",
        "runner/abi/register_endpoint/mod.rs",
        "runner/execution/mod.rs",
    ];

    // When: conventional module declarations are inspected.
    let overrides = facades
        .iter()
        .filter_map(|path| {
            std::fs::read_to_string(source.join(path))
                .ok()
                .filter(|contents| contents.contains("#[path"))
                .map(|_| *path)
        })
        .collect::<Vec<_>>();

    // Then: no facade hides a different physical owner.
    assert!(
        overrides.is_empty(),
        "device facades still override module paths: {overrides:?}"
    );
}
