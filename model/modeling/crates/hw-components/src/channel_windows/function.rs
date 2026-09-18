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
        let window = self.windows.route(address)?;
        if window.owner() != self.function {
            return Err(AccessError::Denied);
        }
        window.read(address, out, downstream)
    }

    /// Writes only the complete data aperture assigned to this Function.
    pub fn write<E>(
        &self,
        address: usize,
        bytes: &[u8],
        downstream: impl FnOnce(usize, &[u8]) -> Result<(), E>,
    ) -> Result<(), AccessError<E>> {
        let window = self.windows.route(address)?;
        if window.owner() != self.function {
            return Err(AccessError::Denied);
        }
        window.write(address, bytes, downstream)
    }
}
