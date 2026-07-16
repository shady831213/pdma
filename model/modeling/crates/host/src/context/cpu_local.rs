use super::{CpuIdentity, active_cpu};
use pdma_model_contract::{ModelError, ModelResult};
use std::{fmt, sync::Arc};
use tokio::sync::Mutex;

/// One mutable value bound to a modeled software CPU.
///
/// Cloning this capability shares the same value; it does not clone `T`.
/// Access is synchronous and limited to the closure passed to [`Self::with`].
/// A reference therefore cannot escape:
///
/// ```compile_fail
/// use pdma_model_host::SoftwareCpu;
///
/// let cpu = SoftwareCpu::new();
/// let local = cpu.local(String::from("state"));
/// let leaked: &str = local.with(|state| state.as_str()).unwrap();
/// # let _ = leaked;
/// ```
///
/// Nor can a future retain a borrow across an await:
///
/// ```compile_fail
/// use pdma_model_host::SoftwareCpu;
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
    owner: Arc<CpuIdentity>,
    state: Arc<Mutex<T>>,
}

impl<T> CpuLocal<T> {
    pub(super) fn new(owner: Arc<CpuIdentity>, value: T) -> Self {
        Self {
            owner,
            state: Arc::new(Mutex::new(value)),
        }
    }

    pub(crate) fn capture_active(value: T) -> ModelResult<Self> {
        let owner = active_cpu().ok_or(ModelError::SoftwareCpuInactive)?;
        Ok(Self::new(owner, value))
    }

    /// Applies one synchronous operation while the owning CPU is active.
    pub fn with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> ModelResult<R> {
        let active = active_cpu().ok_or(ModelError::SoftwareCpuInactive)?;
        if !Arc::ptr_eq(&active, &self.owner) {
            return Err(ModelError::SoftwareCpuMismatch);
        }
        let mut state = self
            .state
            .try_lock()
            .map_err(|_| ModelError::SoftwareCpuStateBusy)?;
        Ok(operation(&mut state))
    }
}

impl<T> Clone for CpuLocal<T> {
    fn clone(&self) -> Self {
        Self {
            owner: Arc::clone(&self.owner),
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> fmt::Debug for CpuLocal<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("CpuLocal").finish_non_exhaustive()
    }
}
