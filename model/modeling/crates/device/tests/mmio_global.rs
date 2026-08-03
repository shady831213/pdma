#![allow(missing_docs)]

#[path = "mmio_global/commands.rs"]
pub mod commands;
#[path = "mmio_global/states.rs"]
pub mod states;
#[path = "mmio_global/support.rs"]
pub mod support;

#[tokio::test(flavor = "current_thread")]
async fn global_commands_expose_canonical_typed_values_and_preserve_prior_response()
-> support::TestResult {
    commands::assert_global_commands().await
}

#[tokio::test(flavor = "current_thread")]
async fn status_distinguishes_all_current_work_and_pressure_classes_without_mutation()
-> support::TestResult {
    states::assert_run_state_matrix().await
}

#[test]
fn worker_and_status_query_use_the_same_current_state_probe() {
    let step = include_str!("../src/runner/native/step.rs");
    let global = include_str!("../src/runner/mmio/global.rs");
    assert!(step.contains("probe_run_state("));
    assert!(global.contains("probe_run_state("));
    assert!(!global.contains("last_status"));
}
