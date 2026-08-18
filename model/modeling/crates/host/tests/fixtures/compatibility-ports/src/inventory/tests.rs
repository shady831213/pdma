use super::{HEADER, is_complete, render};

#[test]
fn manifest_rejects_missing_row() {
    let manifest = render().expect("workspace source must be readable");
    assert!(is_complete(&manifest));
    let mut lines = manifest.lines().collect::<Vec<_>>();
    lines.pop().expect("manifest must contain live rows");
    assert!(!is_complete(&format!("{}\n", lines.join("\n"))));
}

#[test]
fn manifest_rejects_duplicate_row() {
    let manifest = render().expect("workspace source must be readable");
    let row = manifest
        .lines()
        .last()
        .expect("manifest must contain live rows");
    assert!(!is_complete(&format!("{manifest}{row}\n")));
}

#[test]
fn manifest_rejects_stale_row() {
    let manifest = render().expect("workspace source must be readable");
    let stale = format!(
        "{manifest}task26\tcrates/host/src/lib.rs\t999999\tRetiredRoot\tcanonical\tnone\tremoved\n"
    );
    assert!(!is_complete(&stale));
}

#[test]
fn manifest_rejects_uninventoried_row_and_bad_header() {
    let manifest = render().expect("workspace source must be readable");
    let uninventoried = format!(
        "{manifest}task26-host-compat\tcrates/host/src/lib.rs\t1\tGhostPort\tnone\tnone\tnone\n"
    );
    assert!(!is_complete(&uninventoried));
    assert!(!is_complete(&manifest.replacen(HEADER, "bad-header", 1)));
}
