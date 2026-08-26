//! Contract source paths must mirror their public Rust module ownership.

use std::path::Path;

#[test]
fn contract_layers_follow_their_rust_module_paths() {
    // Given: the contract source root and its public layer/module ownership.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let required = [
        "semantic/mod.rs",
        "semantic/completion.rs",
        "semantic/desc/mod.rs",
        "semantic/desc/broadcast.rs",
        "semantic/desc/copy.rs",
        "semantic/desc/vector_add.rs",
        "semantic/dma.rs",
        "semantic/event.rs",
        "semantic/ids.rs",
        "semantic/irq.rs",
        "semantic/queue.rs",
        "semantic/resources.rs",
        "semantic/trace.rs",
        "abi/mod.rs",
        "abi/control.rs",
        "binding/mod.rs",
        "binding/control.rs",
        "binding/control/queue.rs",
        "binding/event.rs",
        "binding/ring.rs",
        "translation/mod.rs",
        "translation/command.rs",
        "translation/error.rs",
        "translation/record.rs",
        "translation/status.rs",
    ];
    let retired = [
        "semantic.rs",
        "completion.rs",
        "desc.rs",
        "desc",
        "dma.rs",
        "event.rs",
        "ids.rs",
        "irq.rs",
        "queue.rs",
        "resources.rs",
        "trace.rs",
        "abi.rs",
        "bridge.rs",
        "bridge",
    ];

    // When: the physical tree is compared with the public Rust module tree.
    let missing = required
        .iter()
        .filter(|path| !source.join(path).exists())
        .copied()
        .collect::<Vec<_>>();
    let flattened = retired
        .iter()
        .filter(|path| source.join(path).exists())
        .copied()
        .collect::<Vec<_>>();

    // Then: every layer is physically nested and no flattened alias remains.
    assert!(
        missing.is_empty(),
        "missing module-owned paths: {missing:?}"
    );
    assert!(
        flattened.is_empty(),
        "flattened module paths remain: {flattened:?}"
    );
}

#[test]
fn contract_layer_facades_need_no_path_overrides() {
    // Given: each facade that owns one public contract layer.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let facades = [
        "semantic/mod.rs",
        "semantic/desc/mod.rs",
        "abi/mod.rs",
        "binding/mod.rs",
        "translation/mod.rs",
    ];

    // When: their module declarations are read.
    let overrides = facades
        .iter()
        .filter_map(|path| {
            std::fs::read_to_string(source.join(path))
                .ok()
                .filter(|contents| contents.contains("#[path"))
                .map(|_| *path)
        })
        .collect::<Vec<_>>();

    // Then: conventional module discovery is sufficient.
    assert!(
        overrides.is_empty(),
        "facades still override module paths: {overrides:?}"
    );
}
