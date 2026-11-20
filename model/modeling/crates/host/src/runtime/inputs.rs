//! Platform adapter for routing assertions of independent DMA level inputs.
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::Dma;

/// Samples each physical input independently for a platform with message delivery.
/// A held level produces one message until software deasserts and rearms the input.
#[derive(Debug)]
pub struct IrqInputs {
    channels: Vec<bool>,
    fatal: bool,
}

impl IrqInputs {
    /// Connect initially low controller inputs to this DMA instance.
    pub fn new<M, S>(dma: &Dma<M, S>) -> Self {
        Self {
            channels: vec![false; dma.channel_irqs().len()],
            fatal: false,
        }
    }

    /// Capture newly asserted sources before routing them to their current owners.
    pub fn sample<M, S>(
        &mut self,
        dma: &Dma<M, S>,
        mut channel: impl FnMut(ChannelId),
        fatal: impl FnOnce(),
    ) {
        for (index, (previous, level)) in
            self.channels.iter_mut().zip(dma.channel_irqs()).enumerate()
        {
            if !std::mem::replace(previous, level) && level {
                channel(ChannelId::new(index));
            }
        }
        let level = dma.device_irq();
        if !std::mem::replace(&mut self.fatal, level) && level {
            fatal();
        }
    }
}
