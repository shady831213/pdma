#![allow(missing_docs)]

mod commands;
mod states;
mod support;

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
