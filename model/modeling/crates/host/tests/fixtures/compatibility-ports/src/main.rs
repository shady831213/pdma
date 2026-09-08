use std::path::Path;

mod compiler;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let successor_paths_compile = compiler::accepts(&root.join("positive/canonical_paths.rs"));
    let final_error_names_compile = compiler::accepts(&root.join("positive/final_error_names.rs"));
    let raw_devices_rejected =
        compiler::rejects(&root.join("negative/raw_devices.rs"), "the trait bound");
    let manual_irq_rejected = compiler::rejects(
        &root.join("negative/manual_irq.rs"),
        "no method named `service_irq_on`",
    );
    let retired_roots_rejected =
        compiler::rejects(&root.join("negative/retired_roots.rs"), "unresolved import");
    let transitional_error_aliases_rejected = compiler::rejects_all(
        &root.join("negative/transitional_error_aliases.rs"),
        &["unresolved imports", "BusControlError", "MmioBatchError"],
    );
    let compatibility_rejected =
        compiler::rejects(&root.join("negative/compatibility.rs"), "unresolved import");
    assert!(
        successor_paths_compile
            && final_error_names_compile
            && raw_devices_rejected
            && manual_irq_rejected
            && retired_roots_rejected
            && transitional_error_aliases_rejected
            && compatibility_rejected
    );
    println!(
        "raw_devices_rejected={raw_devices_rejected};successor_paths_compile={successor_paths_compile};compatibility_rejected={compatibility_rejected}"
    );
}
