//! Instance-configured channels sharing one DMA execution path.

mod arbitration;
mod build;
mod channel;
mod error;
mod execution;
mod fetch;
mod reset;
mod routing;
mod stages;
#[cfg(test)]
mod tests;

pub use arbitration::RoundRobin;
pub use build::BuildError;
pub use channel::{Channel, CommandRing, ResponseRing};
pub use error::{Error, Fault};
pub use execution::{
    COPY_CHUNK_BYTES, CheckLengths, CopyState, LengthCheck, NextChunk, ReadCopy, Transfer,
    WriteCopy,
};
pub use fetch::Fetch;
use hardware_interface_schema::{RegisterAccessError, RegisterWindow, register_dispatch};
use pdma_model_contract::{
    ChannelBlock, ChannelId, DeviceControl, DeviceControlValues, DeviceFaultCode, DeviceRegisters,
};
pub use reset::{ChannelTask, Resettable};
pub use routing::{ResponsePort, RouteResponse};
pub use stages::{
    CommandLoop, CopyLoop, Execution, Intake, Pipeline, Publish, Responses, RunCommand, UpdateIrq,
};

/// Hardware context; the assembler chooses and owns the memory backing.
#[derive(Debug)]
pub struct Device<M> {
    channels: Vec<Channel>,
    /// Injected memory access capability, never a device-selected backend.
    pub memory: M,
    control: RegisterWindow<DeviceControl>,
    interrupt: bool,
}

impl<M> Device<M> {
    /// Whether hardware has emitted a notification not yet taken by the platform.
    pub fn irq(&self) -> bool {
        self.channels.iter().any(|channel| channel.irq) || self.interrupt
    }

    /// Transfers coalesced notification ownership to the platform interrupt controller.
    pub fn take_interrupt(&mut self) -> bool {
        let mut pending = std::mem::take(&mut self.interrupt);
        for channel in &mut self.channels {
            pending |= std::mem::take(&mut channel.irq);
        }
        pending
    }

    /// Signals a device-wide internal hardware fault, never a software bus command.
    pub fn signal_fatal(&mut self) {
        if self.control.values.fault.code() == Ok(DeviceFaultCode::None) {
            self.control.values.fault = self
                .control
                .values
                .fault
                .with_code(DeviceFaultCode::Internal);
            self.interrupt |= self.control.values.irq_enable.raw() != 0;
        }
    }

    pub(crate) fn halted(&self) -> bool {
        self.control.values.fault.code() != Ok(DeviceFaultCode::None)
    }

    /// Borrows one channel and shared hardware resources for a single interaction.
    pub fn channel_context(&mut self, id: ChannelId) -> Result<ChannelContext<'_, M>, Error> {
        let device_fatal = self.halted();
        Ok(ChannelContext {
            channel: self.channels.get_mut(id.index()).ok_or(Error::Unmapped)?,
            memory: &self.memory,
            device_fatal,
        })
    }

    /// Reads a register and applies the owning channel's immediate side effects.
    pub fn read(&mut self, offset: usize, destination: &mut [u8]) -> Result<(), Error> {
        register_dispatch!(
            self.control
                .read(offset, destination)
                .map_err(RegisterAccessError::widen),
            self.read_channels(offset, destination),
        )
        .map_err(Error::from)
    }

    /// Writes a register and applies the owning channel's immediate side effects.
    pub fn write(&mut self, offset: usize, source: &[u8]) -> Result<(), Error> {
        register_dispatch!(
            self.write_control(offset, source),
            self.write_channels(offset, source),
        )
        .map_err(Error::from)
    }

    fn write_control(
        &mut self,
        address: usize,
        source: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let channels = &mut self.channels;
        let interrupt = &mut self.interrupt;
        self.control.write_with(
            address,
            source,
            |_, _, _| Ok(()),
            |_, next| {
                if next.reset.raw() != 0 {
                    for channel in channels {
                        channel.reset();
                    }
                    *next = DeviceControlValues {
                        channel_num: next.channel_num,
                        ..DeviceControlValues::default()
                    };
                    *interrupt = false;
                }
            },
        )
    }

    fn read_channels(
        &mut self,
        address: usize,
        destination: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let (channel, offset) = self.channel_registers(address)?;
        register_dispatch!(
            channel.read(offset, destination),
            Err(RegisterAccessError::InvalidOffset { offset }),
        )
    }

    fn write_channels(
        &mut self,
        address: usize,
        source: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let (channel, offset) = self.channel_registers(address)?;
        register_dispatch!(
            channel.write(offset, source),
            Err(RegisterAccessError::InvalidOffset { offset }),
        )
    }

    fn channel_registers(
        &mut self,
        address: usize,
    ) -> Result<(&mut Channel, usize), RegisterAccessError<Error>> {
        let relative = address
            .checked_sub(DeviceRegisters::CHANNELS_OFFSET)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        let stride = DeviceRegisters::CHANNEL_STRIDE;
        let index = relative / stride;
        let offset = relative % stride;
        let count = self.channels.len();
        let channel = self
            .channels
            .get_mut(index)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        if index + 1 == count && offset >= ChannelBlock::EXTENT {
            return Err(RegisterAccessError::OutsideWindow { address });
        }
        Ok((channel, offset))
    }
}

/// One interaction's selected channel; no borrow escapes a stage step.
#[derive(Debug)]
pub struct ChannelContext<'a, M> {
    /// The channel whose operation is being advanced.
    pub channel: &'a mut Channel,
    /// Shared externally supplied memory access.
    pub memory: &'a M,
    device_fatal: bool,
}

impl<M> ChannelContext<'_, M> {
    /// Whether this channel or the shared device has faulted.
    pub fn halted(&self) -> bool {
        self.device_fatal || self.channel.halted()
    }
}
