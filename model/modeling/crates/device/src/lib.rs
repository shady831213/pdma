//! Instance-configured channels sharing one DMA execution path.

mod build;
mod channel;
mod error;
mod execution;
mod fetch;
mod multi_channel;
mod reset;
mod stages;

pub use build::BuildError;
pub use channel::{Channel, CommandRing, ResponseRing};
pub use error::{Error, Fault};
pub use execution::{
    COPY_CHUNK_BYTES, CheckLengths, CopyState, LengthCheck, NextChunk, ReadCopy, Transfer,
    WriteCopy,
};
pub use fetch::Fetch;
use hardware_interface_schema::{Access, RegisterMapLayout};
pub use multi_channel::{MultiChannel, RunCommand};
use pdma_model_contract::{
    ChannelId, DeviceControl, DeviceControlValues, DeviceFaultCode, DeviceRegisters,
};
pub use reset::{ChannelTask, Resettable};
pub use stages::{CopyLoop, Execution, Pipeline, Publish, UpdateIrq};

/// Hardware context; the assembler chooses and owns the memory backing.
#[derive(Debug)]
pub struct Device<M> {
    channels: Vec<Channel>,
    /// Injected memory access capability, never a device-selected backend.
    pub memory: M,
    control: DeviceControlValues,
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
        if self.control.fault.code() == Ok(DeviceFaultCode::None) {
            self.control.fault = self.control.fault.with_code(DeviceFaultCode::Internal);
            self.interrupt |= self.control.irq_enable.raw() != 0;
        }
    }

    pub(crate) fn halted(&self) -> bool {
        self.control.fault.code() != Ok(DeviceFaultCode::None)
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
        if offset < DeviceRegisters::CHANNELS_OFFSET {
            read_register::<DeviceControl>(
                &self.control,
                offset - DeviceRegisters::CONTROL.offset(),
                destination,
            )
        } else {
            let relative = offset - DeviceRegisters::CHANNELS_OFFSET;
            let stride = DeviceRegisters::CHANNEL_STRIDE;
            self.channels
                .get_mut(relative / stride)
                .ok_or(Error::Unmapped)?
                .read(relative % stride, destination)
        }
    }

    /// Writes a register and applies the owning channel's immediate side effects.
    pub fn write(&mut self, offset: usize, source: &[u8]) -> Result<(), Error> {
        if offset < DeviceRegisters::CHANNELS_OFFSET {
            write_register::<DeviceControl>(
                &mut self.control,
                offset - DeviceRegisters::CONTROL.offset(),
                source,
            )?;
            if self.control.reset.raw() != 0 {
                for channel in &mut self.channels {
                    channel.reset();
                }
                let channel_num = self.control.channel_num;
                self.control = DeviceControlValues {
                    channel_num,
                    ..DeviceControlValues::default()
                };
                self.interrupt = false;
            }
            Ok(())
        } else {
            let relative = offset - DeviceRegisters::CHANNELS_OFFSET;
            let stride = DeviceRegisters::CHANNEL_STRIDE;
            self.channels
                .get_mut(relative / stride)
                .ok_or(Error::Unmapped)?
                .write(relative % stride, source)
        }
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
fn read_register<M: RegisterMapLayout>(
    values: &M::Value,
    offset: usize,
    out: &mut [u8],
) -> Result<(), Error> {
    let definition = M::register_definition(offset).ok_or(Error::Unmapped)?;
    if out.len() != definition.width() {
        return Err(Error::Width);
    }
    if definition.access() == Access::WriteOnly {
        return Err(Error::Access);
    }
    let raw = M::read_register(values, offset).ok_or(Error::Unmapped)?;
    definition
        .view_mut(out)
        .and_then(|mut view| view.set_value(raw))
        .map_err(|_| Error::Width)
}

fn write_register<M: RegisterMapLayout>(
    values: &mut M::Value,
    offset: usize,
    bytes: &[u8],
) -> Result<(), Error> {
    let definition = M::register_definition(offset).ok_or(Error::Unmapped)?;
    if bytes.len() != definition.width() {
        return Err(Error::Width);
    }
    if definition.access() == Access::ReadOnly {
        return Err(Error::Access);
    }
    let raw = definition
        .view(bytes)
        .and_then(|view| view.value())
        .map_err(|_| Error::Width)?;
    let old = M::read_register(values, offset).ok_or(Error::Unmapped)?;
    if (raw ^ old) & !definition.writable_mask() != 0 {
        return Err(Error::ReservedBits);
    }
    M::write_register(values, offset, raw).ok_or(Error::Width)
}
