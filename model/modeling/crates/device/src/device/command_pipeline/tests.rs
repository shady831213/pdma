#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "checked unit-test fixtures"
)]

mod adverse;
mod no_replay;
mod parity;
mod support;

#[test]
fn drive_policy_parity() {
    parity::drive_policy_parity();
}

#[test]
fn no_replay() {
    no_replay::no_replay();
}
