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
    ChannelId, DeviceControl, DeviceFaultCode, DeviceRegistersLayout, ResetValues,
};
pub use reset::{ChannelTask, ResetControl, Resettable};
pub use routing::{ResponsePort, RouteResponse};
pub use stages::{
    CommandLoop, CopyLoop, Execution, Intake, Pipeline, Publish, Responses, RunCommand, UpdateIrq,
};

/// Hardware context; the assembler chooses and owns the memory backing.
#[derive(Debug)]
pub struct Device<M> {
    registers: DeviceRegistersLayout,
    channels: Vec<Channel>,
    /// Injected memory access capability, never a device-selected backend.
    pub memory: M,
    control: RegisterWindow<DeviceControl>,
    interrupt: bool,
    active: Option<(ChannelId, u64)>,
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

    pub(crate) fn resetting(&self) -> bool {
        self.control.values.reset.request()
    }

    pub(crate) fn request_reset(&mut self) {
        if !self.resetting() {
            self.control.values.reset = ResetValues::new(1);
            for channel in &mut self.channels {
                channel.request_reset();
            }
        }
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
        let resetting = self.resetting();
        let mut requested = false;
        self.control.write_with(
            address,
            source,
            |offset, _, _| {
                if resetting && offset != DeviceControl::RESET.offset() {
                    return Err(Error::ResetInProgress);
                }
                Ok(())
            },
            |offset, next| {
                if offset == DeviceControl::RESET.offset() {
                    requested = next.reset.request();
                    next.reset = ResetValues::new(u32::from(resetting));
                }
            },
        )?;
        if requested {
            self.request_reset();
        }
        Ok(())
    }

    fn read_channels(
        &mut self,
        address: usize,
        destination: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let bank = self.registers.channels();
        register_dispatch!(
            bank.route(address, |index, offset| {
                self.channels
                    .get_mut(index)
                    .ok_or(RegisterAccessError::OutsideWindow { address })?
                    .read(offset, destination)
            }),
            Err(bank.finish_access(address)),
        )
    }

    fn write_channels(
        &mut self,
        address: usize,
        source: &[u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        let bank = self.registers.channels();
        register_dispatch!(
            bank.route(address, |index, offset| {
                self.channels
                    .get_mut(index)
                    .ok_or(RegisterAccessError::OutsideWindow { address })?
                    .write(offset, source)
            }),
            Err(bank.finish_access(address)),
        )
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
