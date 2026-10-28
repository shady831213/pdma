use super::HandlerRegistry;
use crate::{EnvContextError, SoftwareEnv};
use hw_model_irq::{InterruptEnvelope, InterruptHandler};
use std::{error::Error, fmt};
use tokio::sync::Mutex;

/// Software's ISR registration capability, borrowed for one runtime invocation.
#[derive(Debug)]
pub struct InterruptRegistration<'a, N: InterruptEnvelope, H> {
    pub(super) env: &'a SoftwareEnv,
    pub(super) registry: &'a Mutex<HandlerRegistry<N::Vector, N, H>>,
}

/// Rejected ISR registration; an existing handler is never replaced.
#[derive(Debug, PartialEq, Eq)]
pub enum InterruptRegistrationError<V> {
    /// Registration was attempted outside its environment context.
    Env(EnvContextError),
    /// This vector already has a handler.
    Duplicate(V),
}

impl<V: fmt::Debug> fmt::Display for InterruptRegistrationError<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Env(error) => error.fmt(f),
            Self::Duplicate(vector) => {
                write!(f, "interrupt vector {vector:?} is already registered")
            }
        }
    }
}

impl<V: fmt::Debug> Error for InterruptRegistrationError<V> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Env(error) => Some(error),
            Self::Duplicate(_) => None,
        }
    }
}

impl<N: InterruptEnvelope, H: InterruptHandler<N>> InterruptRegistration<'_, N, H>
where
    N::Vector: Ord,
{
    /// Installs a persistent ISR on the running program's environment.
    ///
    /// On rejection the supplied handler is dropped; existing registrations stay intact.
    pub fn register(
        &self,
        vector: N::Vector,
        handler: H,
    ) -> Result<(), InterruptRegistrationError<N::Vector>> {
        self.env
            .validate_active()
            .map_err(InterruptRegistrationError::Env)?;
        self.registry
            .try_lock()
            .map_err(|_| InterruptRegistrationError::Env(EnvContextError::StateBusy))?
            .register(vector.clone(), handler)
            .map_err(|_| InterruptRegistrationError::Duplicate(vector))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    struct Notice;
    impl InterruptEnvelope for Notice {
        type Vector = u8;
        fn vector(&self) -> u8 {
            0
        }
    }
    struct Handler;
    impl InterruptHandler<Notice> for Handler {
        type Error = Infallible;
        async fn handle(&mut self, _: &Notice) -> Result<(), Infallible> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn registration_requires_the_active_runtime_env() {
        // Given: a registration capability bound to an inactive environment.
        let registry = Mutex::new(HandlerRegistry::new());
        let env = SoftwareEnv::new();
        let registration = InterruptRegistration {
            env: &env,
            registry: &registry,
        };
        // When: registration is attempted outside that environment.
        let result = registration.register(0, Handler);
        // Then: the handler table remains empty.
        assert_eq!(
            result,
            Err(InterruptRegistrationError::Env(EnvContextError::Inactive))
        );
        assert!(registry.lock().await.is_empty());
    }
}
