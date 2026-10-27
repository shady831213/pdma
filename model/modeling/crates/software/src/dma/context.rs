use super::{CHANNEL, ChannelControl, Error};
use hardware_interface_io::register::RegisterAccess;
use pdma_model_contract::dma::ChannelConfiguration;
use pdma_model_contract::dma::{ChannelRegisters, ContextId};

impl ChannelControl {
    /// Selects ring memory before first enable; hardware rejects changes while live.
    pub async fn set_ring_context<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        context: ContextId,
    ) -> Result<(), Error<B::Error>> {
        self.channel_write(
            bus,
            CHANNEL
                + ChannelRegisters::CONFIG.offset()
                + ChannelConfiguration::RING_CONTEXT.offset(),
            &context.raw().to_le_bytes(),
        )
        .await
    }
}
