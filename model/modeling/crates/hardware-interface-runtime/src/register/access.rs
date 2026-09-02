use std::error::Error;

use super::{
    RegisterBusAddress, RegisterBusBase, RegisterBusError, RegisterBusOffset, RegisterBusWidth,
};

/// Asynchronous exact-width access to a numeric register address space.
pub trait RegisterAccess {
    /// Endpoint-owned typed access failure.
    type Error: Error + Send + Sync + 'static;

    /// Reads exactly `response.len()` bytes into caller-owned storage.
    fn read(
        &self,
        offset: u32,
        response: &mut [u8],
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Writes exactly `request.len()` caller-owned bytes.
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
