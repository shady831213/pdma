use super::Channel;
use super::Error;
use hw_model_io::register::RegisterAccess;
use pdma_model_contract::dma::{
    ChannelFatalCode, ChannelIrqCauseValues, ChannelIrqMaskValues, DmaFatalCode,
};

impl<B: RegisterAccess + Send + Sync, M: Send + Sync> Channel<B, M> {
    /// Reports whether a payload fault is waiting for software repair and resume.
    pub async fn faulted(&self) -> Result<bool, Error<B::Error>> {
        self.port.control.faulted(&self.port.bus).await
    }

    /// Requests retry after mapping repair; serialize with fault handling, reset and release.
    pub async fn resume(&self) -> Result<(), Error<B::Error>> {
        self.port.control.resume(&self.port.bus).await
    }

    /// Reads the hardware's sticky channel fatal code.
    pub async fn fatal(&self) -> Result<ChannelFatalCode, Error<B::Error>> {
        self.port.control.fatal(&self.port.bus).await
    }

    /// Enables or masks all channel interrupt notifications.
    ///
    /// Enabling asserts IRQ immediately for any active, unmasked cause.
    pub async fn set_irq_enabled(&self, enabled: bool) -> Result<(), Error<B::Error>> {
        self.port
            .control
            .set_irq_enabled(&self.port.bus, enabled)
            .await
    }

    /// Masks event-nonempty and Trap notifications; errors and fatals bypass this mask.
    ///
    /// Clearing a mask exposes an active cause immediately when IRQ is enabled.
    pub async fn set_irq_mask(&self, mask: ChannelIrqMaskValues) -> Result<(), Error<B::Error>> {
        self.port.control.set_irq_mask(&self.port.bus, mask).await
    }

    /// Reads channel causes regardless of interrupt masks without acknowledging them.
    pub async fn irq_cause(&self) -> Result<ChannelIrqCauseValues, Error<B::Error>> {
        self.port.control.irq_cause(&self.port.bus).await
    }

    /// Acknowledges the sticky Trap cause without retracting pending notifications.
    pub async fn acknowledge_trap(&self) -> Result<(), Error<B::Error>> {
        self.port.control.acknowledge_trap(&self.port.bus).await
    }

    /// Requests channel reset and waits for disabled, unconfigured hardware.
    ///
    /// Success means all prior command, list, payload and event memory accesses have
    /// drained and no later access can belong to that work. External memory is preserved;
    /// construct a fresh driver with `Self::setup` before reuse. Hardware must run independently.
    ///
    /// # Cancel safety
    /// Dropping this future, including through a caller timeout, does not cancel an accepted
    /// hardware request. A timeout is not completion: keep its memory valid until reset reads zero.
    pub async fn reset(self) -> Result<(), Error<B::Error>> {
        self.port.control.reset(&self.port.bus).await
    }

    /// Reads the sticky device-wide fatal, which channel reset cannot clear.
    pub async fn dma_fatal(&self) -> Result<DmaFatalCode, Error<B::Error>> {
        self.port.control.dma_fatal(&self.port.bus).await
    }

    /// Enables or masks device-fatal notifications independently of the channel gate.
    ///
    /// Enabling asserts the device IRQ if device fatal is active.
    pub async fn set_dma_irq_enabled(&self, enabled: bool) -> Result<(), Error<B::Error>> {
        self.port
            .control
            .set_dma_irq_enabled(&self.port.bus, enabled)
            .await
    }

    /// Requests device reset and waits for every channel to become disabled and unconfigured.
    ///
    /// Success drains all prior command, list, payload and event memory accesses across
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
