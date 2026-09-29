use super::ChannelWindows;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;

/// Translates channel metadata to its register-owned Function before forwarding memory access.
#[derive(Debug)]
pub struct ChannelMemoryAccess<'a, M: ?Sized> {
    windows: &'a ChannelWindows,
    downstream: &'a M,
}

impl ChannelWindows {
    /// Borrows the owner bank and a Function-tagged downstream port without owning its backing.
    pub const fn memory<'a, M: ?Sized>(&'a self, downstream: &'a M) -> ChannelMemoryAccess<'a, M> {
        ChannelMemoryAccess {
            windows: self,
            downstream,
        }
    }
}

impl<M: ?Sized> ChannelMemoryAccess<'_, M> {
    fn owner<E>(&self, channel: ChannelId) -> Result<FunctionId, MemoryError<E>> {
        self.windows
            .owner(channel)
            .ok_or(MemoryError::UnownedChannel { channel })
    }
}

impl<M: MemoryAccess<FunctionId> + ?Sized> MemoryAccess<ChannelId> for ChannelMemoryAccess<'_, M> {
    type Error = MemoryError<M::Error>;
    fn validate_with_sideband(
        &self,
        channel: &ChannelId,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        let function = self.owner(*channel)?;
        self.downstream
            .validate_with_sideband(&function, address, alignment, length)
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
            .read_with_sideband(&function, address, alignment, bytes)
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
            .write_with_sideband(&function, address, alignment, bytes)
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
