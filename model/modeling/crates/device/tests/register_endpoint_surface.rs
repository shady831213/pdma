//! Task 18 exact register endpoint surface and adverse validation.

#[path = "register_endpoint_surface/adverse.rs"]
/// Adverse endpoint surface.
pub mod adverse;
#[path = "register_endpoint_surface/order_reset.rs"]
/// Clone ordering and reset surface.
pub mod order_reset;
#[path = "register_endpoint_surface/support.rs"]
/// Shared surface fixtures.
pub mod support;
#[path = "register_endpoint_surface/surface.rs"]
/// Exact happy endpoint surface.
pub mod surface;

#[tokio::test(flavor = "current_thread")]
async fn task18_register_endpoint_surface() -> support::TestResult {
    surface::run().await
}

#[tokio::test(flavor = "current_thread")]
async fn task18_register_endpoint_adverse() -> support::TestResult {
    adverse::run().await
}
