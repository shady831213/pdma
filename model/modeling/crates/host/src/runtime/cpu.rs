#[cfg(test)]
use hardware_interface_runtime::SoftwareCpu as RuntimeSoftwareCpu;
#[cfg(test)]
use std::future::Future;

mod cpu_local;
#[cfg(test)]
mod tests;

use cpu_local::CpuLocal;

/// Compatibility adapter for the runtime software CPU.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
struct SoftwareCpu(RuntimeSoftwareCpu);

#[cfg(test)]
impl SoftwareCpu {
    /// Creates one idle modeled CPU.
    fn new() -> Self {
        Self(RuntimeSoftwareCpu::new())
    }

    /// Binds one mutable value to this modeled CPU.
    fn local<T>(&self, value: T) -> CpuLocal<T> {
        CpuLocal::from_runtime(self.0.local(value))
    }

    /// Runs one complete SW-main program as atomic poll-to-pending segments.
    async fn run_main<F, T>(&self, program: F) -> T
    where
        F: Future<Output = T>,
    {
        self.0.run_main(program).await
    }

    #[cfg(test)]
    async fn run_irq<F, T>(&self, handler: F) -> T
    where
        F: Future<Output = T>,
    {
        self.0.run_handler_turn(handler).await
    }
}
