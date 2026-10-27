use super::ChannelWindows;
use hardware_interface_io::memory::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAddress, MemoryAlignment,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;

/// Translates channel metadata to its register-owned Function before forwarding memory access.
#[derive(Debug)]
pub struct ChannelMemoryAccess<'a, M: ?Sized, F = fn(ChannelId, FunctionId) -> FunctionId> {
    windows: &'a ChannelWindows,
    downstream: &'a M,
    map: F,
}

impl ChannelWindows {
    /// Borrows the owner bank and a Function-tagged downstream port without owning its backing.
    pub const fn memory<'a, M: ?Sized>(&'a self, downstream: &'a M) -> ChannelMemoryAccess<'a, M> {
        ChannelMemoryAccess {
            windows: self,
            downstream,
            map: |_, function| function,
        }
    }
}

impl<'a, M: ?Sized> ChannelMemoryAccess<'a, M> {
    /// Maps an authorized channel and owner to platform sideband on every memory operation.
    #[must_use]
    pub fn map_sideband<F, S>(self, map: F) -> ChannelMemoryAccess<'a, M, F>
    where
        F: Fn(ChannelId, FunctionId) -> S,
    {
        ChannelMemoryAccess {
            windows: self.windows,
            downstream: self.downstream,
            map,
        }
    }
}

impl<M: ?Sized, F> ChannelMemoryAccess<'_, M, F> {
    fn owner<E>(&self, channel: ChannelId) -> Result<FunctionId, MemoryError<E>> {
        self.windows
            .owner(channel)
            .ok_or(MemoryError::UnownedChannel { channel })
    }
}

impl<M: MemoryAccess<S> + ?Sized, F, S> MemoryAccess<ChannelId> for ChannelMemoryAccess<'_, M, F>
where
    F: Fn(ChannelId, FunctionId) -> S,
{
    type Error = MemoryError<M::Error>;
    fn atomic32_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        let identity = self.owner(*channel).map_err(AtomicAccessError::Access)?;
        self.downstream
            .atomic32_with_sideband(&(self.map)(*channel, identity), address, operation)
            .map_err(|error| match error {
                AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                AtomicAccessError::Access(error) => {
                    AtomicAccessError::Access(MemoryError::Downstream(error))
                }
            })
    }

    fn atomic64_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        let identity = self.owner(*channel).map_err(AtomicAccessError::Access)?;
        self.downstream
            .atomic64_with_sideband(&(self.map)(*channel, identity), address, operation)
            .map_err(|error| match error {
                AtomicAccessError::Unsupported => AtomicAccessError::Unsupported,
                AtomicAccessError::Access(error) => {
                    AtomicAccessError::Access(MemoryError::Downstream(error))
                }
            })
    }
    fn validate_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        let function = self.owner(*channel)?;
        self.downstream
            .validate_with_sideband(&(self.map)(*channel, function), address, alignment, length)
            .map_err(MemoryError::Downstream)
    }

    fn read_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        let function = self.owner(*channel)?;
        self.downstream
            .read_with_sideband(&(self.map)(*channel, function), address, alignment, bytes)
            .map_err(MemoryError::Downstream)
    }

    fn write_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        let function = self.owner(*channel)?;
        self.downstream
            .write_with_sideband(&(self.map)(*channel, function), address, alignment, bytes)
            .map_err(MemoryError::Downstream)
    }
}

/// Rejection by this hardware memory port or a preserved downstream failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError<E> {
    /// No owner exists for this channel selection.
    UnownedChannel {
        /// Channel requested by the upstream access.
        channel: ChannelId,
    },
    /// Failure reported by the selected downstream memory.
    Downstream(E),
}

impl<E: core::fmt::Display> core::fmt::Display for MemoryError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnownedChannel { channel } => {
                write!(f, "channel {} has no memory owner", channel.index())
            }
            Self::Downstream(error) => error.fmt(f),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for MemoryError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Downstream(error) => Some(error),
            Self::UnownedChannel { .. } => None,
        }
    }
}

impl<E: Into<Self>> From<MemoryError<E>> for pdma_model_dma::MemoryFailure {
    fn from(error: MemoryError<E>) -> Self {
        match error {
            MemoryError::Downstream(error) => error.into(),
            MemoryError::UnownedChannel { .. } => Self::Access,
        }
    }
}
