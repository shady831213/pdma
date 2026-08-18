use crate::ModelError;
use crate::ModelResult;
use hardware_interface_runtime::{CpuContextError, CpuLocal as RuntimeCpuLocal};
use std::fmt;

/// Compatibility adapter for runtime CPU-local state.
///
/// Cloning shares the same value and access remains synchronous. A reference cannot escape:
///
/// ```compile_fail
/// use hardware_interface_runtime::SoftwareCpu;
///
/// let cpu = SoftwareCpu::new();
/// let local = cpu.local(String::from("state"));
/// let leaked: &str = local.with(|state| state.as_str()).unwrap();
/// # let _ = leaked;
/// ```
///
/// A future cannot retain a borrow across an await:
///
/// ```compile_fail
/// use hardware_interface_runtime::SoftwareCpu;
///
/// let cpu = SoftwareCpu::new();
/// let local = cpu.local(String::from("state"));
/// let future = local.with(|state| async move {
///     tokio::task::yield_now().await;
///     state.push('!');
/// });
/// # let _ = future;
/// ```
pub(super) struct CpuLocal<T>(RuntimeCpuLocal<T>);

impl<T> CpuLocal<T> {
    #[cfg(test)]
    pub(super) const fn from_runtime(local: RuntimeCpuLocal<T>) -> Self {
        Self(local)
    }

    /// Applies one synchronous operation while the owning CPU is active.
    pub(super) fn with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> ModelResult<R> {
        self.0.with(operation).map_err(map_context_error)
    }
}

const fn map_context_error(error: CpuContextError) -> ModelError {
    match error {
        CpuContextError::Inactive => ModelError::SoftwareCpuInactive,
        CpuContextError::Mismatch => ModelError::SoftwareCpuMismatch,
        CpuContextError::StateBusy => ModelError::SoftwareCpuStateBusy,
    }
}

impl<T> Clone for CpuLocal<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> fmt::Debug for CpuLocal<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("CpuLocal").finish_non_exhaustive()
    }
}
