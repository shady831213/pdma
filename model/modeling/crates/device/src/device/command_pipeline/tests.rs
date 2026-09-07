#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "checked unit-test fixtures"
)]

mod adverse;
mod no_replay;
mod parity;
mod support;

#[tokio::test(flavor = "current_thread")]
async fn drive_policy_parity() {
    parity::drive_policy_parity().await;
}

#[test]
fn no_replay() {
    no_replay::no_replay();
}
