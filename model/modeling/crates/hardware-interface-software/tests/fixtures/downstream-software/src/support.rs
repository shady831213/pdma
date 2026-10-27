mod error;
mod interrupt;
mod register;

use hir::TraceBuffer;

pub async fn run_register(
    trace: &TraceBuffer<&'static str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Ok(register::run_register(trace).await?)
}

pub async fn run_happy_dispatch(
    trace: &TraceBuffer<&'static str>,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    Ok(interrupt::run_happy_dispatch(trace).await?)
}

pub async fn run_adverse_dispatch() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Ok(interrupt::run_adverse_dispatch().await?)
}
