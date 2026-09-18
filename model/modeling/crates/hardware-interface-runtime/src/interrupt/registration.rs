use super::{HandlerRegistry, InterruptEnvelope, InterruptHandler};
use crate::{CpuContextError, SoftwareCpu};
use std::{error::Error, fmt};
use tokio::sync::Mutex;

/// Software's ISR registration capability, borrowed for one runtime invocation.
#[derive(Debug)]
pub struct InterruptRegistration<'a, N: InterruptEnvelope, H> {
    pub(super) cpu: &'a SoftwareCpu,
    pub(super) registry: &'a Mutex<HandlerRegistry<N::Vector, N, H>>,
}

/// Rejected ISR registration; an existing handler is never replaced.
#[derive(Debug, PartialEq, Eq)]
pub enum InterruptRegistrationError<V> {
    /// Registration was attempted outside its CPU context.
    Cpu(CpuContextError),
    /// This vector already has a handler.
    Duplicate(V),
}

impl<V: fmt::Debug> fmt::Display for InterruptRegistrationError<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu(error) => error.fmt(f),
            Self::Duplicate(vector) => {
                write!(f, "interrupt vector {vector:?} is already registered")
            }
        }
    }
}

impl<V: fmt::Debug> Error for InterruptRegistrationError<V> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::Duplicate(_) => None,
        }
    }
}

impl<N: InterruptEnvelope, H: InterruptHandler<N>> InterruptRegistration<'_, N, H> {
    /// Installs a persistent ISR on the running program's CPU.
    ///
    /// On rejection the supplied handler is dropped; existing registrations stay intact.
    pub fn register(
        &self,
        vector: N::Vector,
        handler: H,
    ) -> Result<(), InterruptRegistrationError<N::Vector>> {
        self.cpu
            .validate_active()
            .map_err(InterruptRegistrationError::Cpu)?;
        self.registry
            .try_lock()
            .map_err(|_| InterruptRegistrationError::Cpu(CpuContextError::StateBusy))?
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
    async fn registration_requires_the_active_runtime_cpu() {
        // Given: a registration capability bound to an inactive CPU.
        let registry = Mutex::new(HandlerRegistry::new());
        let cpu = SoftwareCpu::new();
        let registration = InterruptRegistration {
            cpu: &cpu,
            registry: &registry,
        };
        // When: registration is attempted outside that CPU.
        let result = registration.register(0, Handler);
        // Then: the handler table remains empty.
        assert_eq!(
            result,
            Err(InterruptRegistrationError::Cpu(CpuContextError::Inactive))
        );
        assert!(registry.lock().await.is_empty());
    }
}
