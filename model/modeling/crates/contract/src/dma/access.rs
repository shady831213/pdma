use super::ChannelId;

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
