use std::{
    cell::RefCell,
    error::Error,
    fmt,
    future::{Future, poll_fn},
    pin::pin,
    sync::Arc,
    task::Poll,
};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(super) struct CpuToken;

thread_local! {
    static CURRENT_CPU: RefCell<Option<Arc<CpuToken>>> = const { RefCell::new(None) };
}

/// Failure to use a capability in its owning software CPU context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuContextError {
    /// No software CPU is active for this poll.
    Inactive,
    /// A different software CPU is active for this poll.
    Mismatch,
    /// The CPU-local state is already synchronously borrowed.
    StateBusy,
}

impl fmt::Display for CpuContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inactive => formatter.write_str("software CPU is inactive"),
            Self::Mismatch => formatter.write_str("software CPU does not own this capability"),
            Self::StateBusy => formatter.write_str("CPU-local state is busy"),
        }
    }
}

impl Error for CpuContextError {}

/// Opaque captured ownership used to preflight later nested polls.
#[derive(Debug, Clone)]
pub(super) struct CpuOwner {
    token: Arc<CpuToken>,
}

impl CpuOwner {
    /// Captures the software CPU active for the current inner poll.
    pub(super) fn capture_active() -> Result<Self, CpuContextError> {
        current_token()
            .map(|token| Self { token })
            .ok_or(CpuContextError::Inactive)
    }

    /// Checks that this owner is active for the current inner poll.
    pub(super) fn validate_active(&self) -> Result<(), CpuContextError> {
        let active = current_token().ok_or(CpuContextError::Inactive)?;
        if Arc::ptr_eq(&active, &self.token) {
            Ok(())
        } else {
            Err(CpuContextError::Mismatch)
        }
    }

    /// Checks CPU ownership before every poll of an asynchronous capability.
    pub(super) async fn scope<F: Future>(
        &self,
        operation: F,
    ) -> Result<F::Output, CpuContextError> {
        let mut operation = pin!(operation);
        poll_fn(|context| {
            if let Err(error) = self.validate_active() {
                return Poll::Ready(Err(error));
            }
            operation.as_mut().poll(context).map(Ok)
        })
        .await
    }

    pub(super) const fn from_token(token: Arc<CpuToken>) -> Self {
        Self { token }
    }
}

pub(super) fn current_token() -> Option<Arc<CpuToken>> {
    CURRENT_CPU.with(|current| current.borrow().clone())
}

pub(super) struct ActiveCpu {
    previous: Option<Arc<CpuToken>>,
}

impl ActiveCpu {
    pub(super) fn enter(token: Arc<CpuToken>) -> Self {
        let previous = CURRENT_CPU.with(|current| current.replace(Some(token)));
        Self { previous }
    }
}

impl Drop for ActiveCpu {
    fn drop(&mut self) {
        CURRENT_CPU.with(|current| {
            current.replace(self.previous.take());
        });
    }
}
