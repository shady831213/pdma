#[expect(
    clippy::redundant_pub_crate,
    reason = "The fixture is shared within each integration-test crate, not exported."
)]
pub(crate) mod raw_hardware;
