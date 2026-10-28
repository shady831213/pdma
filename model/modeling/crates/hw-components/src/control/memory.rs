use super::ControlChannel;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::memory::{InvalidContextAttributes, MemoryIdentity};

#[doc = "Borrowed firmware Host-memory port using the independent PF control configuration."]
#[derive(Debug)]
pub struct ControlMemoryAccess<'a, M: ?Sized> {
    control: &'a ControlChannel,
    downstream: &'a M,
}

impl ControlChannel {
    #[doc = "Borrows a fabric port without taking ownership of backing memory."]
    pub const fn memory<'a, M: ?Sized>(&'a self, downstream: &'a M) -> ControlMemoryAccess<'a, M> {
        ControlMemoryAccess {
            control: self,
            downstream,
        }
    }
}

impl<M: ?Sized> ControlMemoryAccess<'_, M> {
    fn identity<E>(&self) -> Result<MemoryIdentity, MemoryError<E>> {
        self.control
            .memory_identity()
            .map_err(MemoryError::InvalidAttributes)
    }
}

impl<M: MemoryAccess<MemoryIdentity> + ?Sized> MemoryAccess for ControlMemoryAccess<'_, M> {
    type Error = MemoryError<M::Error>;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.downstream
            .validate_with_sideband(&self.identity()?, address, alignment, length)
            .map_err(MemoryError::Downstream)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.downstream
            .read_with_sideband(&self.identity()?, address, alignment, bytes)
            .map_err(MemoryError::Downstream)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.downstream
            .write_with_sideband(&self.identity()?, address, alignment, bytes)
            .map_err(MemoryError::Downstream)
    }
}

/// Rejection by this hardware memory port or a preserved downstream failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError<E> {
    /// Control memory attributes do not describe a valid address space.
    InvalidAttributes(InvalidContextAttributes),
    /// Failure reported by the selected downstream memory.
    Downstream(E),
}

impl<E: core::fmt::Display> core::fmt::Display for MemoryError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidAttributes(error) => error.fmt(f),
            Self::Downstream(error) => error.fmt(f),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for MemoryError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Downstream(error) => Some(error),
            Self::InvalidAttributes(error) => Some(error),
        }
    }
}
