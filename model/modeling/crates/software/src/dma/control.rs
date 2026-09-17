use super::{CHANNEL, Error};
use hardware_interface_runtime::{CpuOwner, RegisterAccess};
use pdma_model_contract::{
    ChannelFaultCode, ChannelFaultValues, ChannelId, ChannelIrqCauseValues, ChannelIrqMaskValues,
    ChannelRegisters, DeviceControl, DeviceFaultCode, DeviceFaultValues, DeviceRegisters,
};

const CONTROL: usize = DeviceRegisters::CONTROL.offset();

/// CPU-bound register control without DMA ring ownership or allocation.
#[derive(Debug)]
pub struct ChannelControl {
    pub(super) owner: CpuOwner,
    pub(super) channel_base: usize,
}

impl ChannelControl {
    /// Discovers a hardware channel on the active software CPU.
    pub async fn discover<B: RegisterAccess + Sync>(
        bus: &B,
        id: ChannelId,
    ) -> Result<Self, Error<B::Error>> {
        let owner = CpuOwner::capture_active().map_err(Error::Cpu)?;
        Self::discover_owned(bus, owner, id).await
    }

    pub(super) async fn discover_owned<B: RegisterAccess + Sync>(
        bus: &B,
        owner: CpuOwner,
        id: ChannelId,
    ) -> Result<Self, Error<B::Error>> {
        let mut control = Self {
            owner,
            channel_base: 0,
        };
        let available = control
            .read_word(bus, CONTROL + DeviceControl::CHANNEL_NUM.offset())
            .await?;
        if id.index() >= usize::try_from(available).map_err(|_| Error::Geometry)? {
            return Err(Error::Channel {
                requested: id,
                available,
            });
        }
        control.channel_base = id.register_offset().ok_or(Error::Geometry)?;
        Ok(control)
    }

    /// Reads the hardware's sticky channel fault code.
    pub async fn fault<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
    ) -> Result<ChannelFaultCode, Error<B::Error>> {
        let raw = self
            .channel_read_word(bus, CHANNEL + ChannelRegisters::FAULT.offset())
            .await?;
        ChannelFaultValues::new(raw)
            .code()
            .map_err(Error::RegisterCodec)
    }

    /// Enables or masks all channel interrupt notifications.
    ///
    /// Enabling does not replay masked events; inspect causes and pending responses.
    pub async fn set_irq_enabled<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        enabled: bool,
    ) -> Result<(), Error<B::Error>> {
        self.channel_write(
            bus,
            CHANNEL + ChannelRegisters::IRQ_ENABLE.offset(),
            &u32::from(enabled).to_le_bytes(),
        )
        .await
    }

    /// Masks selected routine response events; error and fatal events bypass this mask.
    ///
    /// Clearing a mask does not replay events; inspect causes and pending responses.
    pub async fn set_irq_mask<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        mask: ChannelIrqMaskValues,
    ) -> Result<(), Error<B::Error>> {
        self.channel_write(
            bus,
            CHANNEL + ChannelRegisters::IRQ_MASK.offset(),
            &mask.raw().to_le_bytes(),
        )
        .await
    }

    /// Reads channel causes regardless of interrupt masks without acknowledging them.
    pub async fn irq_cause<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
    ) -> Result<ChannelIrqCauseValues, Error<B::Error>> {
        self.channel_read_word(bus, CHANNEL + ChannelRegisters::IRQ_CAUSE.offset())
            .await
            .map(ChannelIrqCauseValues::new)
    }

    /// Requests channel reset and waits for disabled, unconfigured hardware.
    ///
    /// Success means all prior command, list, payload and response memory accesses have
    /// drained and no later access can belong to that work. External memory is preserved;
    /// construct a fresh ring driver before reuse. Hardware must run independently.
    ///
    /// # Cancel safety
    /// Dropping this future, including through a caller timeout, does not cancel an accepted
    /// hardware request. A timeout is not completion: keep its memory valid until reset reads zero.
    pub async fn reset<B: RegisterAccess + Sync>(&self, bus: &B) -> Result<(), Error<B::Error>> {
        self.request_reset(bus).await?;
        self.wait_reset(bus).await
    }

    /// Requests reset without claiming that accepted hardware work has drained.
    pub async fn request_reset<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
    ) -> Result<(), Error<B::Error>> {
        self.channel_write(
            bus,
            CHANNEL + ChannelRegisters::RESET.offset(),
            &1_u32.to_le_bytes(),
        )
        .await
    }

    /// Waits for hardware cleanup; cancellation leaves its request and memory obligations intact.
    pub async fn wait_reset<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
    ) -> Result<(), Error<B::Error>> {
        while self.reset_in_progress(bus).await? {
            tokio::task::yield_now().await;
        }
        Ok(())
    }

    /// Samples hardware reset busy without waiting or changing its request.
    pub async fn reset_in_progress<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
    ) -> Result<bool, Error<B::Error>> {
        self.channel_read_word(bus, CHANNEL + ChannelRegisters::RESET.offset())
            .await
            .map(|busy| busy != 0)
    }

    /// Reads the sticky device-wide fault, which channel reset cannot clear.
    pub async fn device_fault<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
    ) -> Result<DeviceFaultCode, Error<B::Error>> {
        let raw = self
            .read_word(bus, CONTROL + DeviceControl::FAULT.offset())
            .await?;
        DeviceFaultValues::new(raw)
            .code()
            .map_err(Error::RegisterCodec)
    }

    /// Enables or masks device-fatal notifications independently of the channel gate.
    ///
    /// Enabling does not replay masked events; inspect the device fault afterward.
    pub async fn set_device_irq_enabled<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        enabled: bool,
    ) -> Result<(), Error<B::Error>> {
        self.write(
            bus,
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
    pub async fn reset_device<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
    ) -> Result<(), Error<B::Error>> {
        self.write(
            bus,
            CONTROL + DeviceControl::RESET.offset(),
            &1_u32.to_le_bytes(),
        )
        .await?;
        while self
            .read_word(bus, CONTROL + DeviceControl::RESET.offset())
            .await?
            != 0
        {
            tokio::task::yield_now().await;
        }
        Ok(())
    }
}
