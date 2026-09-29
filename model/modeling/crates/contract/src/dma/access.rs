use super::registers::DmaRegisters;

/// A channel selection, checked against hardware discovery by its consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(usize);

impl ChannelId {
    /// Creates a channel selection without assuming a device's channel count.
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based channel index.
    pub const fn index(self) -> usize {
        self.0
    }

    /// Returns the data aperture for a valid population and selection, or `None` on overflow.
    pub fn register_offset(self, channel_count: usize) -> Option<usize> {
        DmaRegisters::new(channel_count)
            .ok()?
            .channels()
            .offset(self.0)
    }

    /// Returns the privileged recovery address for a valid population and selection.
    pub fn admin_offset(self, channel_count: usize) -> Option<usize> {
        let layout = DmaRegisters::new(channel_count).ok()?;
        layout
            .admin()
            .base()
            .checked_add(layout.admin().child().channels().offset(self.0)?)
    }
}

/// An allocated context selector, distinct from a PASID or task identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContextId(u32);

impl ContextId {
    /// Creates a selector; the receiving table validates allocation and ownership.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    /// Returns the descriptor/register encoding.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// DMA-owned selectors resolved by the surrounding hardware, not by the DMA engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaAccess {
    /// Channel whose owner authorizes the access.
    pub channel: ChannelId,
    /// Ring, SG, source or destination binding selected for this access.
    pub context: ContextId,
}
