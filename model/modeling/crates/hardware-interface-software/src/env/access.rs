use super::{EnvContextError, EnvOwner, SoftwareEnv};
use hardware_interface_io::register::RegisterAccess;
use std::{error::Error, fmt};

/// Environment ownership or underlying register-access failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvAccessError<E> {
    /// The owning environment was not active for this poll.
    Env(EnvContextError),
    /// The underlying register transport rejected the access.
    Access(E),
}

impl<E: fmt::Display> fmt::Display for EnvAccessError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Env(error) => error.fmt(formatter),
            Self::Access(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + 'static> Error for EnvAccessError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Env(error) => Some(error),
            Self::Access(error) => Some(error),
        }
    }
}

/// Register access checked against its owning environment before every transport poll.
#[derive(Debug, Clone)]
pub struct EnvRegisterAccess<B> {
    owner: EnvOwner,
    bus: B,
}

impl SoftwareEnv {
    /// Binds registers to this environment without requiring an active environment context.
    pub fn bind_registers<B: RegisterAccess>(&self, bus: B) -> EnvRegisterAccess<B> {
        EnvRegisterAccess {
            owner: self.owner(),
            bus,
        }
    }
}

impl<B: RegisterAccess + Sync> RegisterAccess for EnvRegisterAccess<B> {
    type Error = EnvAccessError<B::Error>;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.owner
            .scope(async { self.bus.read(offset, response).await })
            .await
            .map_err(EnvAccessError::Env)?
            .map_err(EnvAccessError::Access)
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.owner
            .scope(async { self.bus.write(offset, request).await })
            .await
            .map_err(EnvAccessError::Env)?
            .map_err(EnvAccessError::Access)
    }
}
