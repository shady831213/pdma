use super::{CHANNEL, Channel};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::{
    ChannelFaultCode, ChannelFaultValues, ChannelIrqCauseValues, ChannelIrqMaskValues,
    ChannelRegisters, DeviceControl, DeviceFaultCode, DeviceFaultValues, DeviceRegisters,
};

const CONTROL: usize = DeviceRegisters::CONTROL.offset();

impl<B: RegisterAccess + Send + Sync, M: MemoryAccess + Send + Sync> Channel<B, M> {
    /// Reads the hardware's sticky channel fault code.
    pub async fn fault(&self) -> Result<ChannelFaultCode, Error<B::Error>> {
        let raw = self
            .port
            .channel_read_word(CHANNEL + ChannelRegisters::FAULT.offset())
            .await?;
        ChannelFaultValues::new(raw)
            .code()
            .map_err(Error::RegisterCodec)
    }

    /// Enables or masks all channel interrupt notifications.
    ///
    /// Enabling does not replay masked events; inspect causes and pending responses.
    pub async fn set_irq_enabled(&self, enabled: bool) -> Result<(), Error<B::Error>> {
        self.port
            .channel_write(
                CHANNEL + ChannelRegisters::IRQ_ENABLE.offset(),
                &u32::from(enabled).to_le_bytes(),
            )
            .await
    }

    /// Masks selected routine response events; error and fatal events bypass this mask.
    ///
    /// Clearing a mask does not replay events; inspect causes and pending responses.
    pub async fn set_irq_mask(&self, mask: ChannelIrqMaskValues) -> Result<(), Error<B::Error>> {
        self.port
            .channel_write(
                CHANNEL + ChannelRegisters::IRQ_MASK.offset(),
                &mask.raw().to_le_bytes(),
            )
            .await
    }

    /// Reads channel causes regardless of interrupt masks without acknowledging them.
    pub async fn irq_cause(&self) -> Result<ChannelIrqCauseValues, Error<B::Error>> {
        self.port
            .channel_read_word(CHANNEL + ChannelRegisters::IRQ_CAUSE.offset())
            .await
            .map(ChannelIrqCauseValues::new)
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
        self.port
            .channel_write(
                CHANNEL + ChannelRegisters::RESET.offset(),
                &1_u32.to_le_bytes(),
            )
            .await?;
        while self
            .port
            .channel_read_word(CHANNEL + ChannelRegisters::RESET.offset())
            .await?
            != 0
        {
            tokio::task::yield_now().await;
        }
        Ok(())
    }

    /// Reads the sticky device-wide fault, which channel reset cannot clear.
    pub async fn device_fault(&self) -> Result<DeviceFaultCode, Error<B::Error>> {
        let raw = self
            .port
            .read_word(CONTROL + DeviceControl::FAULT.offset())
            .await?;
        DeviceFaultValues::new(raw)
            .code()
            .map_err(Error::RegisterCodec)
    }

    /// Enables or masks device-fatal notifications independently of the channel gate.
    ///
    /// Enabling does not replay masked events; inspect the device fault afterward.
    pub async fn set_device_irq_enabled(&self, enabled: bool) -> Result<(), Error<B::Error>> {
        self.port
            .write(
                CONTROL + DeviceControl::IRQ_ENABLE.offset(),
                &u32::from(enabled).to_le_bytes(),
            )
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
    pub async fn reset_device(self) -> Result<(), Error<B::Error>> {
        self.port
            .write(
                CONTROL + DeviceControl::RESET.offset(),
                &1_u32.to_le_bytes(),
            )
            .await?;
        while self
            .port
            .read_word(CONTROL + DeviceControl::RESET.offset())
            .await?
            != 0
        {
            tokio::task::yield_now().await;
        }
        Ok(())
    }
}
