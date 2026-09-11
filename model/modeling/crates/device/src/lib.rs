//! Minimal single-channel DMA hardware.

mod channel;
mod error;
mod execution;
mod reset;
mod stages;

pub use channel::{Channel, CommandRing, ResponseRing};
pub use error::{Error, Fault};
pub use execution::{
    COPY_CHUNK_BYTES, CheckLengths, CopyState, LengthCheck, NextChunk, ReadCopy, Transfer,
    WriteCopy,
};
use hardware_interface_schema::{Access, RegisterMapLayout};
use pdma_model_contract::{DeviceControl, DeviceControlValues, DeviceFaultCode, DeviceRegisters};
pub use reset::{ResetState, Resettable};
pub use stages::{CopyLoop, Execution, Fetch, Pipeline, Publish, UpdateIrq, pipeline};

/// Hardware context; the assembler chooses and owns the memory backing.
#[derive(Debug)]
pub struct Device<M> {
    /// The single channel and its register-owning rings.
    pub channel: Channel,
    /// Injected memory access capability, never a device-selected backend.
    pub memory: M,
    control: DeviceControlValues,
    interrupt: bool,
}

impl<M> Device<M> {
    /// Creates disabled hardware without allocating any external memory.
    pub fn new(memory: M) -> Self {
        Self {
            channel: Channel::default(),
            memory,
            control: DeviceControlValues::default(),
            interrupt: false,
        }
    }

    /// Whether hardware has emitted a notification not yet taken by the platform.
    pub const fn irq(&self) -> bool {
        self.channel.irq || self.interrupt
    }

    /// Transfers coalesced notification ownership to the platform interrupt controller.
    pub fn take_interrupt(&mut self) -> bool {
        std::mem::take(&mut self.channel.irq) | std::mem::take(&mut self.interrupt)
    }

    /// Signals a device-wide internal hardware fault, never a software bus command.
    pub fn signal_fatal(&mut self) {
        if self.control.fault.code() == Ok(DeviceFaultCode::None) {
            self.control.fault = self.control.fault.with_code(DeviceFaultCode::Internal);
            self.interrupt |= self.control.irq_enable.raw() != 0;
        }
    }

    pub(crate) fn halted(&self) -> bool {
        self.control.fault.code() != Ok(DeviceFaultCode::None) || self.channel.halted()
    }

    /// Reads a register and applies the owning channel's immediate side effects.
    pub fn read(&mut self, offset: usize, destination: &mut [u8]) -> Result<(), Error> {
        if offset >= DeviceRegisters::CONTROL.offset() {
            read_register::<DeviceControl>(
                &self.control,
                offset - DeviceRegisters::CONTROL.offset(),
                destination,
            )
        } else {
            self.channel.read(offset, destination)
        }
    }

    /// Writes a register and applies the owning channel's immediate side effects.
    pub fn write(&mut self, offset: usize, source: &[u8]) -> Result<(), Error> {
        if offset >= DeviceRegisters::CONTROL.offset() {
            write_register::<DeviceControl>(
                &mut self.control,
                offset - DeviceRegisters::CONTROL.offset(),
                source,
            )?;
            if self.control.reset.raw() != 0 {
                self.channel.reset();
                self.control = DeviceControlValues::default();
                self.interrupt = false;
            }
            Ok(())
        } else {
            self.channel.write(offset, source)
        }
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
