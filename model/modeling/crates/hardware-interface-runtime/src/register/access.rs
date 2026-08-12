use std::error::Error;

/// Asynchronous exact-width access to a numeric register address space.
pub trait RegisterAccess {
    /// Endpoint-owned typed access failure.
    type Error: Error + Send + Sync + 'static;

    /// Reads exactly `response.len()` bytes into caller-owned storage.
    fn read(
        &self,
        offset: u32,
        response: &mut [u8],
    ) -> impl Future<Output = Result<(), Self::Error>>;

    /// Writes exactly `request.len()` caller-owned bytes.
    fn write(&self, offset: u32, request: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}
