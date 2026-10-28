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
pub(super) struct EnvToken;

thread_local! {
    static CURRENT_ENV: RefCell<Option<Arc<EnvToken>>> = const { RefCell::new(None) };
}

/// Failure to use a capability in its owning software environment context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvContextError {
    /// No software environment is active for this poll.
    Inactive,
    /// A different software environment is active for this poll.
    Mismatch,
    /// The environment-local state is already synchronously borrowed.
    StateBusy,
}

impl fmt::Display for EnvContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inactive => formatter.write_str("software environment is inactive"),
            Self::Mismatch => {
                formatter.write_str("software environment does not own this capability")
            }
            Self::StateBusy => formatter.write_str("environment-local state is busy"),
        }
    }
}

impl Error for EnvContextError {}

/// Opaque captured ownership used to preflight later nested polls.
#[derive(Debug, Clone)]
pub(super) struct EnvOwner {
    token: Arc<EnvToken>,
}

impl EnvOwner {
    /// Captures the software environment active for the current inner poll.
    pub(super) fn capture_active() -> Result<Self, EnvContextError> {
        current_token()
            .map(|token| Self { token })
            .ok_or(EnvContextError::Inactive)
    }

    /// Checks that this owner is active for the current inner poll.
    pub(super) fn validate_active(&self) -> Result<(), EnvContextError> {
        let active = current_token().ok_or(EnvContextError::Inactive)?;
        if Arc::ptr_eq(&active, &self.token) {
            Ok(())
        } else {
            Err(EnvContextError::Mismatch)
        }
    }

    /// Checks environment ownership before every poll of an asynchronous capability.
    pub(super) async fn scope<F: Future>(
        &self,
        operation: F,
    ) -> Result<F::Output, EnvContextError> {
        let mut operation = pin!(operation);
        poll_fn(|context| {
            if let Err(error) = self.validate_active() {
                return Poll::Ready(Err(error));
            }
            operation.as_mut().poll(context).map(Ok)
        })
        .await
    }

    pub(super) const fn from_token(token: Arc<EnvToken>) -> Self {
        Self { token }
    }
}

pub(super) fn current_token() -> Option<Arc<EnvToken>> {
    CURRENT_ENV.with(|current| current.borrow().clone())
}

pub(super) struct ActiveEnv {
    previous: Option<Arc<EnvToken>>,
}

impl ActiveEnv {
    pub(super) fn enter(token: Arc<EnvToken>) -> Self {
        let previous = CURRENT_ENV.with(|current| current.replace(Some(token)));
        Self { previous }
    }
}

impl Drop for ActiveEnv {
    fn drop(&mut self) {
        CURRENT_ENV.with(|current| {
            current.replace(self.previous.take());
        });
    }
}
