use super::ContextTable;
use crate::windows::ChannelWindows;
use hardware_interface_runtime::{
    AtomicAccessError, AtomicOperation, MemoryAccess, MemoryAddress, MemoryAlignment,
};
use pdma_model_contract::dma::{ChannelId, ContextId, DmaAccess};
use pdma_model_contract::memory::{FunctionId, MemoryIdentity};

#[doc = "Borrowed context lookup translating DMA selectors into authorized fabric identities."]
#[derive(Debug)]
pub struct ContextMemoryAccess<'a, M: ?Sized, F = fn(DmaAccess, MemoryIdentity) -> MemoryIdentity> {
    table: &'a ContextTable,
    windows: &'a ChannelWindows,
    downstream: &'a M,
    map: F,
}

impl ContextTable {
    #[doc = "Borrows the actual owner registers and platform fabric without owning memory backing."]
    pub const fn memory<'a, M: ?Sized>(
        &'a self,
        windows: &'a ChannelWindows,
        downstream: &'a M,
    ) -> ContextMemoryAccess<'a, M> {
        ContextMemoryAccess {
            table: self,
            windows,
            downstream,
            map: |_, identity| identity,
        }
    }
}

impl<'a, M: ?Sized> ContextMemoryAccess<'a, M> {
    /// Maps authorized selectors and identity to platform sideband on every memory operation.
    #[must_use]
    pub fn map_sideband<F, S>(self, map: F) -> ContextMemoryAccess<'a, M, F>
    where
        F: Fn(DmaAccess, MemoryIdentity) -> S,
    {
        ContextMemoryAccess {
            table: self.table,
            windows: self.windows,
            downstream: self.downstream,
            map,
        }
    }
}

impl<M: ?Sized, F> ContextMemoryAccess<'_, M, F> {
    fn identity<E>(&self, access: DmaAccess) -> Result<MemoryIdentity, MemoryError<E>> {
        let owner = self
            .windows
            .owner(access.channel)
            .ok_or(MemoryError::UnownedChannel {
                channel: access.channel,
            })?;
        let entry = usize::try_from(access.context.raw())
            .ok()
            .and_then(|index| self.table.entries.get(index))
            .filter(|entry| entry.values.valid.enabled())
            .ok_or(MemoryError::InvalidContext {
                context: access.context,
            })?;
        if FunctionId::new(entry.values.owner.raw()) != owner {
            return Err(MemoryError::OwnerMismatch {
                context: access.context,
                owner,
            });
        }
        Ok(MemoryIdentity {
            function: owner,
            attributes: entry.values.attributes.attributes().map_err(|_| {
                MemoryError::InvalidAttributes {
                    context: access.context,
                }
            })?,
        })
    }
}

impl<M: MemoryAccess<S> + ?Sized, F, S> MemoryAccess<DmaAccess> for ContextMemoryAccess<'_, M, F>
where
    F: Fn(DmaAccess, MemoryIdentity) -> S,
{
    type Error = MemoryError<M::Error>;
    fn atomic32_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        let identity = self.identity(*access).map_err(AtomicAccessError::Access)?;
        self.downstream
            .atomic32_with_sideband(&(self.map)(*access, identity), address, operation)
            .map_err(|error| error.map_access(MemoryError::Downstream))
    }

    fn atomic64_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        let identity = self.identity(*access).map_err(AtomicAccessError::Access)?;
        self.downstream
            .atomic64_with_sideband(&(self.map)(*access, identity), address, operation)
            .map_err(|error| error.map_access(MemoryError::Downstream))
    }
    fn validate_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.downstream
            .validate_with_sideband(
                &(self.map)(*access, self.identity(*access)?),
                address,
                alignment,
                length,
            )
            .map_err(MemoryError::Downstream)
    }
    fn read_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.downstream
            .read_with_sideband(
                &(self.map)(*access, self.identity(*access)?),
                address,
                alignment,
                bytes,
            )
            .map_err(MemoryError::Downstream)
    }
    fn write_with_sideband(
        &self,
        access: &DmaAccess,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.downstream
            .write_with_sideband(
                &(self.map)(*access, self.identity(*access)?),
                address,
                alignment,
                bytes,
            )
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
    /// The context selector does not identify a valid entry.
    InvalidContext {
        /// Context selector that failed the validity check.
        context: ContextId,
    },
    /// The selected context belongs to another Function.
    OwnerMismatch {
        /// Context selected by the upstream access.
        context: ContextId,
        /// Channel owner that must also own the context.
        owner: FunctionId,
    },
    /// The context's address-space attributes are invalid.
    InvalidAttributes {
        /// Context whose address-space attributes could not be decoded.
        context: ContextId,
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
            Self::InvalidContext { context } => write!(f, "context {} is not valid", context.raw()),
            Self::OwnerMismatch { context, owner } => write!(
                f,
                "context {} is not owned by Function {}",
                context.raw(),
                owner.raw()
            ),
            Self::InvalidAttributes { context } => {
                write!(f, "context {} has invalid attributes", context.raw())
            }
            Self::Downstream(error) => error.fmt(f),
        }
    }
}

impl<E: core::error::Error + 'static> core::error::Error for MemoryError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Downstream(error) => Some(error),
            Self::UnownedChannel { .. }
            | Self::InvalidContext { .. }
            | Self::OwnerMismatch { .. }
            | Self::InvalidAttributes { .. } => None,
        }
    }
}

impl<E: Into<Self>> From<MemoryError<E>> for pdma_model_dma::MemoryFailure {
    fn from(error: MemoryError<E>) -> Self {
        match error {
            MemoryError::Downstream(error) => error.into(),
            MemoryError::UnownedChannel { .. }
            | MemoryError::InvalidContext { .. }
            | MemoryError::OwnerMismatch { .. }
            | MemoryError::InvalidAttributes { .. } => Self::Access,
        }
    }
}
