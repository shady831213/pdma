use std::error::Error;

use super::{
    RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterBusOffset, RegisterBusWidth,
};

/// Asynchronous exact-width access to a numeric register address space.
///
/// `offset` is a byte offset relative to the endpoint. The buffer length is the
/// requested access width; it is not permission to split or coalesce accesses.
/// Endpoints define valid widths, register semantics, side effects and cancellation
/// behavior. In particular, dropping a future does not imply rollback.
/// Use [`AddressedRegisterAccess`] to add checked base/address translation.
pub trait RegisterAccess {
    /// Endpoint-owned typed access failure.
    type Error: Error + Send + Sync + 'static;

    /// Reads exactly `response.len()` bytes into caller-owned storage.
    ///
    /// # Errors
    /// Returns the endpoint's access error. Whether an unsuccessful operation has
    /// modified the destination or caused read side effects is endpoint-defined.
    fn read(
        &self,
        offset: u32,
        response: &mut [u8],
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Writes exactly `request.len()` caller-owned bytes.
    ///
    /// # Errors
    /// Returns the endpoint's access error; effects and retry safety are defined
    /// by the endpoint, not by this transport trait.
    fn write(
        &self,
        offset: u32,
        request: &[u8],
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Additive full-address access derived from a checked numeric register offset endpoint.
pub trait AddressedRegisterAccess: RegisterAccess {
    /// Reads exactly `response.len()` bytes after checked base-plus-address translation.
    fn read_addressed(
        &self,
        base: RegisterBusBase,
        address: RegisterBusAddress,
        response: &mut [u8],
    ) -> impl Future<Output = Result<(), RegisterBusError<Self::Error>>> + Send
    where
        Self: Sync,
    {
        async move {
            let width = RegisterBusWidth::new(response.len());
            let offset = RegisterBusOffset::from_address(base, address, width)
                .map_err(RegisterBusError::<Self::Error>::from_translation)?;
            self.read(offset.raw(), response)
                .await
                .map_err(|source| RegisterBusError::Endpoint {
                    base,
                    address,
                    width,
                    source,
                })
        }
    }

    /// Writes exactly `request.len()` bytes after checked base-plus-address translation.
    fn write_addressed(
        &self,
        base: RegisterBusBase,
        address: RegisterBusAddress,
        request: &[u8],
    ) -> impl Future<Output = Result<(), RegisterBusError<Self::Error>>> + Send
    where
        Self: Sync,
    {
        async move {
            let width = RegisterBusWidth::new(request.len());
            let offset = RegisterBusOffset::from_address(base, address, width)
                .map_err(RegisterBusError::<Self::Error>::from_translation)?;
            self.write(offset.raw(), request)
                .await
                .map_err(|source| RegisterBusError::Endpoint {
                    base,
                    address,
                    width,
                    source,
                })
        }
    }
}

impl<T: RegisterAccess + ?Sized> AddressedRegisterAccess for T {}
