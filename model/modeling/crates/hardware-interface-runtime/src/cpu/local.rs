use super::context::{CpuContextError, CpuOwner};
use std::{fmt, sync::Arc};
use tokio::sync::Mutex;

/// Clone-shared mutable state bound to one software CPU.
///
/// Access is synchronous and scoped to [`Self::with`], so a reference cannot escape:
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
/// A future also cannot retain the mutable borrow across an await:
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
pub struct CpuLocal<T> {
    owner: CpuOwner,
    state: Arc<Mutex<T>>,
}

impl<T> CpuLocal<T> {
    pub(super) fn new(owner: CpuOwner, value: T) -> Self {
        Self {
            owner,
            state: Arc::new(Mutex::new(value)),
        }
    }

    /// Captures the active CPU and binds `value` to it.
    pub fn capture_active(value: T) -> Result<Self, CpuContextError> {
        Ok(Self::new(CpuOwner::capture_active()?, value))
    }

    /// Applies one synchronous operation while the owning CPU is active.
    pub fn with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> Result<R, CpuContextError> {
        self.owner.validate_active()?;
        let mut state = self
            .state
            .try_lock()
            .map_err(|_| CpuContextError::StateBusy)?;
        Ok(operation(&mut state))
    }
}

impl<T> Clone for CpuLocal<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> fmt::Debug for CpuLocal<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("CpuLocal").finish_non_exhaustive()
    }
}
