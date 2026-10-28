use super::context::{EnvContextError, EnvOwner};
use std::{fmt, sync::Arc};
use tokio::sync::Mutex;

/// Clone-shared mutable state bound to one software environment.
///
/// Access is synchronous and scoped to [`Self::with`], so a reference cannot escape:
///
/// ```compile_fail
/// use hw_model_swenv::SoftwareEnv;
///
/// let env = SoftwareEnv::new();
/// let local = env.local(String::from("state"));
/// let leaked: &str = local.with(|state| state.as_str()).unwrap();
/// # let _ = leaked;
/// ```
///
/// A future also cannot retain the mutable borrow across an await:
///
/// ```compile_fail
/// use hw_model_swenv::SoftwareEnv;
///
/// let env = SoftwareEnv::new();
/// let local = env.local(String::from("state"));
/// let future = local.with(|state| async move {
///     tokio::task::yield_now().await;
///     state.push('!');
/// });
/// # let _ = future;
/// ```
pub struct EnvLocal<T> {
    owner: EnvOwner,
    state: Arc<Mutex<T>>,
}

impl<T> EnvLocal<T> {
    pub(super) fn new(owner: EnvOwner, value: T) -> Self {
        Self {
            owner,
            state: Arc::new(Mutex::new(value)),
        }
    }

    /// Captures the active environment and binds `value` to it.
    ///
    /// # Errors
    /// Returns [`EnvContextError::Inactive`] outside a software execution turn.
    pub fn capture_active(value: T) -> Result<Self, EnvContextError> {
        Ok(Self::new(EnvOwner::capture_active()?, value))
    }

    /// Applies one synchronous operation while the owning environment is active.
    ///
    /// A callback's returned `Err` is an ordinary value; changes are not rolled
    /// back. Clones share the same state and identity.
    ///
    /// # Errors
    /// Returns [`EnvContextError::Inactive`] outside a turn, [`EnvContextError::Mismatch`]
    /// on another environment, or [`EnvContextError::StateBusy`] on overlapping access.
    /// These failures occur before the callback runs.
    pub fn with<R>(&self, operation: impl FnOnce(&mut T) -> R) -> Result<R, EnvContextError> {
        self.owner.validate_active()?;
        let mut state = self
            .state
            .try_lock()
            .map_err(|_| EnvContextError::StateBusy)?;
        Ok(operation(&mut state))
    }
}

impl<T> Clone for EnvLocal<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> fmt::Debug for EnvLocal<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("EnvLocal").finish_non_exhaustive()
    }
}
