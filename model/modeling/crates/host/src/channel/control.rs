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

    /// Resets the channel to disabled, unconfigured state and consumes its stale cursors.
    ///
    /// External memory is preserved. Construct a fresh driver with `Self::setup` before reuse.
    pub async fn reset(self) -> Result<(), Error<B::Error>> {
        self.port
            .channel_write(
                CHANNEL + ChannelRegisters::RESET.offset(),
                &1_u32.to_le_bytes(),
            )
            .await
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

    /// Resets the whole device to disabled, unconfigured state and consumes this driver.
    ///
    /// External memory is preserved. Construct a fresh driver with `Self::setup` before reuse.
    pub async fn reset_device(self) -> Result<(), Error<B::Error>> {
        self.port
            .write(
                CONTROL + DeviceControl::RESET.offset(),
                &1_u32.to_le_bytes(),
            )
            .await
    }
}
