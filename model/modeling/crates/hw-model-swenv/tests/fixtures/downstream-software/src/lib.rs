//! Standalone renamed-facade consumer of runtime capabilities.
//!
//! `EnvLocal` references and futures remain poll-scoped through the renamed facade:
//!
//! ```compile_fail
//! use hir::SoftwareEnv;
//!
//! let env = SoftwareEnv::new();
//! let local = env.local(String::from("state"));
//! let future = local.with(|state| async move {
//!     tokio::task::yield_now().await;
//!     state.push('!');
//! });
//! # let _ = future;
//! ```
//!
//! Synchronous infallible observation cannot be replaced with fallible or async callbacks:
//!
//! ```compile_fail
//! use hw_model_io::register::observation::{RegisterObservation, RegisterObserver};
//! struct Fallible;
//! impl RegisterObserver for Fallible {
//!     fn observe(&self, _: RegisterObservation) -> Result<(), ()> { Ok(()) }
//! }
//! ```
//!
//! ```compile_fail
//! use hw_model_irq::{InterruptEnvelope};
//! use hir::{InterruptLifecycleEvent, InterruptLifecycleObserver};
//! struct Notice;
//! impl InterruptEnvelope for Notice { type Vector = u8; fn vector(&self) -> u8 { 1 } }
//! struct Asynchronous;
//! impl InterruptLifecycleObserver<Notice> for Asynchronous {
//!     async fn observe(&self, _: InterruptLifecycleEvent<'_, Notice>) {}
//! }
//! ```

mod flow;
#[doc(hidden)]
pub mod support;

pub use flow::{RunReport, run_flow};

async fn run_register(
    trace: &hir::TraceBuffer<&'static str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    support::run_register(trace).await
}

async fn run_happy_dispatch(
    trace: &hir::TraceBuffer<&'static str>,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    support::run_happy_dispatch(trace).await
}

async fn run_adverse_dispatch() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    support::run_adverse_dispatch().await
}
