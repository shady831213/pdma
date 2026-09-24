use super::Channel;
use super::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::dma::{
    ChannelFaultCode, ChannelIrqCauseValues, ChannelIrqMaskValues, DmaFaultCode,
};

impl<B: RegisterAccess + Send + Sync, M: MemoryAccess + Send + Sync> Channel<B, M> {
    /// Reads the hardware's sticky channel fault code.
    pub async fn fault(&self) -> Result<ChannelFaultCode, Error<B::Error>> {
        self.port.control.fault(&self.port.bus).await
    }

    /// Enables or masks all channel interrupt notifications.
    ///
    /// Enabling does not replay masked events; inspect causes and pending responses.
    pub async fn set_irq_enabled(&self, enabled: bool) -> Result<(), Error<B::Error>> {
        self.port
            .control
            .set_irq_enabled(&self.port.bus, enabled)
            .await
    }

    /// Masks selected routine response events; error and fatal events bypass this mask.
    ///
    /// Clearing a mask does not replay events; inspect causes and pending responses.
    pub async fn set_irq_mask(&self, mask: ChannelIrqMaskValues) -> Result<(), Error<B::Error>> {
        self.port.control.set_irq_mask(&self.port.bus, mask).await
    }

    /// Reads channel causes regardless of interrupt masks without acknowledging them.
    pub async fn irq_cause(&self) -> Result<ChannelIrqCauseValues, Error<B::Error>> {
        self.port.control.irq_cause(&self.port.bus).await
    }

    /// Requests channel reset and waits for disabled, unconfigured hardware.
    ///
    /// Success means all prior command, list, payload and response memory accesses have
    /// drained and no later access can belong to that work. External memory is preserved;
    /// construct a fresh driver with `Self::setup` before reuse. Hardware must run independently.
    ///
    /// # Cancel safety
    /// Dropping this future, including through a caller timeout, does not cancel an accepted
    /// hardware request. A timeout is not completion: keep its memory valid until reset reads zero.
    pub async fn reset(self) -> Result<(), Error<B::Error>> {
        self.port.control.reset(&self.port.bus).await
    }

    /// Reads the sticky device-wide fault, which channel reset cannot clear.
    pub async fn dma_fault(&self) -> Result<DmaFaultCode, Error<B::Error>> {
        self.port.control.dma_fault(&self.port.bus).await
    }

    /// Enables or masks device-fatal notifications independently of the channel gate.
    ///
    /// Enabling does not replay masked events; inspect the device fault afterward.
    pub async fn set_dma_irq_enabled(&self, enabled: bool) -> Result<(), Error<B::Error>> {
        self.port
            .control
            .set_dma_irq_enabled(&self.port.bus, enabled)
            .await
    }

    /// Requests device reset and waits for every channel to become disabled and unconfigured.
    ///
    /// Success drains all prior command, list, payload and response memory accesses across
    /// all channels and prevents later accesses by that work. External memory is preserved;
    /// construct fresh drivers before reuse. Hardware must run independently.
    ///
    /// # Cancel safety
    /// Dropping this future, including through a caller timeout, leaves an accepted request
    /// owned by hardware. Keep all affected memory valid until device reset reads zero.
    /// A timeout never indicates successful reset completion.
    pub async fn reset_dma(self) -> Result<(), Error<B::Error>> {
        self.port.control.reset_dma(&self.port.bus).await
    }
}
