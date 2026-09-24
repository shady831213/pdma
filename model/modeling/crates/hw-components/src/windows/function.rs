use super::{AccessError, ChannelWindows, FunctionId};

/// A requester view of the shared hardware permission bank, without shadow state.
#[derive(Clone, Copy, Debug)]
pub struct FunctionWindows<'a> {
    pub(super) windows: &'a ChannelWindows,
    pub(super) function: FunctionId,
}

impl FunctionWindows<'_> {
    /// Reads public discovery or this Function's window; foreign accesses are denied.
    pub fn read<E>(
        &self,
        address: usize,
        out: &mut [u8],
        downstream: impl FnOnce(usize, &mut [u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        if self.windows.discovery.contains(&address) {
            return self.windows.read(address, out, downstream);
        }
        self.windows
            .route(address)?
            .read_for(self.function, (address, out), downstream)
    }

    /// Writes only the complete data aperture assigned to this Function.
    pub fn write<E>(
        &self,
        address: usize,
        bytes: &[u8],
        downstream: impl FnOnce(usize, &[u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        self.windows
            .route(address)?
            .write_for(self.function, (address, bytes), downstream)
    }
}
