use crate::{Error, Fault};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_schema::{Encode, RegisterAccessError, RegisterWindow};
use pdma_model_contract::{ReadWordValues, Response, ResponseRegisters, Ring, RingPointers};

/// Response queue state, including its own generated registers.
#[derive(Debug)]
pub struct ResponseRing {
    pub(crate) registers: RegisterWindow<ResponseRegisters>,
    pub(crate) last_ioc: Option<u32>,
}

impl Default for ResponseRing {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Ring for ResponseRing {
    type Entry = Response;

    fn base(&self) -> u64 {
        self.registers.values.base.raw()
    }

    fn capacity(&self) -> u32 {
        self.registers.values.capacity.raw()
    }
}

impl ResponseRing {
    /// Places this ring's MMIO window in its parent's address space.
    pub const fn new(base: usize) -> Self {
        Self {
            registers: RegisterWindow::new(base),
            last_ioc: None,
        }
    }

    pub(crate) const fn pointers(&self) -> RingPointers {
        RingPointers {
            producer: self.registers.values.producer.raw(),
            consumer: self.registers.values.consumer.raw(),
        }
    }

    pub(crate) fn read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<Error>> {
        self.registers
            .read(address, out)
            .map_err(RegisterAccessError::widen)
    }

    pub(crate) fn write(
        &mut self,
        address: usize,
        bytes: &[u8],
        configured: bool,
        resetting: bool,
    ) -> Result<(), RegisterAccessError<Error>> {
        let pointers = self.pointers();
        let base = self.registers.base();
        let last_ioc = &mut self.last_ioc;
        self.registers.write_with(
            address,
            bytes,
            |_, old, next| {
                if resetting {
                    return Err(Error::ResetInProgress);
                }
                if configured && (next.base != old.base || next.capacity != old.capacity) {
                    return Err(Error::ConfigurationLocked);
                }
                let mut candidate = Self::new(base);
                candidate.registers.values = *next;
                candidate
                    .with_consumer(pointers, next.consumer.raw())
                    .map_err(|_| Error::Cursor)?;
                Ok(())
            },
            |_, next| {
                if last_ioc.is_some_and(|boundary| {
                    boundary.wrapping_sub(pointers.consumer)
                        <= next.consumer.raw().wrapping_sub(pointers.consumer)
                }) {
                    *last_ioc = None;
                }
            },
        )
    }

    pub(crate) fn push(
        &mut self,
        memory: &impl MemoryAccess,
        response: &Response,
    ) -> Result<bool, Fault> {
        let pointers = self.pointers();
        let address = MemoryAddress::new(
            self.entry(pointers.producer)
                .map_err(|_| Fault::ResponseWrite)?,
        );
        if pointers.pending() == self.capacity() {
            return Ok(false);
        }
        let next = self
            .advance_producer(pointers, 1)
            .map_err(|_| Fault::ResponseWrite)?;
        let raw = response.encode().map_err(|_| Fault::ResponseWrite)?;
        memory
            .write_typed(address, MemoryAlignment::ONE, &raw)
            .map_err(|_| Fault::ResponseWrite)?;
        self.registers.values.producer = ReadWordValues::new(next.producer);
        if response.flags.ioc {
            self.last_ioc = Some(next.producer);
        }
        Ok(true)
    }
}
