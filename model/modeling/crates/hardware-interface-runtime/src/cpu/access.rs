use super::{CpuContextError, CpuOwner, SoftwareCpu};
use crate::RegisterAccess;
use std::{error::Error, fmt};

/// CPU ownership or underlying register-access failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuAccessError<E> {
    /// The owning CPU was not active for this poll.
    Cpu(CpuContextError),
    /// The underlying register transport rejected the access.
    Access(E),
}

impl<E: fmt::Display> fmt::Display for CpuAccessError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu(error) => error.fmt(formatter),
            Self::Access(error) => error.fmt(formatter),
        }
    }
}

impl<E: Error + 'static> Error for CpuAccessError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::Access(error) => Some(error),
        }
    }
}

/// Register access checked against its owning CPU before every transport poll.
#[derive(Debug, Clone)]
pub struct CpuRegisterAccess<B> {
    owner: CpuOwner,
    bus: B,
}

impl SoftwareCpu {
    /// Binds registers to this CPU without requiring an active CPU context.
    pub fn bind_registers<B: RegisterAccess>(&self, bus: B) -> CpuRegisterAccess<B> {
        CpuRegisterAccess {
            owner: self.owner(),
            bus,
        }
    }
}

impl<B: RegisterAccess + Sync> RegisterAccess for CpuRegisterAccess<B> {
    type Error = CpuAccessError<B::Error>;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.owner
            .scope(async { self.bus.read(offset, response).await })
            .await
            .map_err(CpuAccessError::Cpu)?
            .map_err(CpuAccessError::Access)
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.owner
            .scope(async { self.bus.write(offset, request).await })
            .await
            .map_err(CpuAccessError::Cpu)?
            .map_err(CpuAccessError::Access)
    }
}
