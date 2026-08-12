use hardware_interface_runtime::SoftwareCpu as RuntimeSoftwareCpu;
use std::future::Future;

mod cpu_local;
#[cfg(test)]
mod tests;

pub use cpu_local::CpuLocal;

/// Compatibility adapter for the runtime software CPU.
#[derive(Debug, Clone, Default)]
pub struct SoftwareCpu(RuntimeSoftwareCpu);

impl SoftwareCpu {
    /// Creates one idle modeled CPU.
    pub fn new() -> Self {
        Self(RuntimeSoftwareCpu::new())
    }

    /// Binds one mutable value to this modeled CPU.
    pub fn local<T>(&self, value: T) -> CpuLocal<T> {
        CpuLocal::from_runtime(self.0.local(value))
    }

    /// Runs one complete SW-main program as atomic poll-to-pending segments.
    pub async fn run_main<F, T>(&self, program: F) -> T
    where
        F: Future<Output = T>,
    {
        self.0.run_main(program).await
    }

    pub(crate) async fn run_irq<F, T>(&self, handler: F) -> T
    where
        F: Future<Output = T>,
    {
        self.0.run_handler_turn(handler).await
    }
}
