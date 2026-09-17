use super::{ControlError, Domain};
use core::marker::PhantomData;
use hardware_interface_schema::{RecordLayout, RegisterAccessError, RegisterWindow};
use pdma_model_contract::{ControlRingRegisters, Ring, RingPointers};

#[derive(Debug)]
pub(super) struct ControlRing<E: RecordLayout> {
    registers: RegisterWindow<ControlRingRegisters>,
    producer: Domain,
    entry: PhantomData<E>,
}

impl<E: RecordLayout> Ring for ControlRing<E> {
    type Entry = E;
    fn base(&self) -> u64 {
        self.registers.values.base.raw()
    }
    fn capacity(&self) -> u32 {
        self.registers.values.capacity.raw()
    }
}

impl<E: RecordLayout> ControlRing<E> {
    pub(super) const fn new(base: usize, producer: Domain) -> Self {
        Self {
            registers: RegisterWindow::new(base),
            producer,
            entry: PhantomData,
        }
    }

    pub(super) fn read(
        &mut self,
        address: usize,
        out: &mut [u8],
    ) -> Result<(), RegisterAccessError<ControlError>> {
        self.registers
            .read(address, out)
            .map_err(RegisterAccessError::widen)
    }

    pub(super) fn write(
        &mut self,
        transaction: (usize, &[u8]),
        domain: Domain,
        enabled: bool,
    ) -> Result<(), RegisterAccessError<ControlError>> {
        let producer = self.producer;
        let pointers = RingPointers {
            producer: self.registers.values.producer.raw(),
            consumer: self.registers.values.consumer.raw(),
        };
        let mut candidate = Self::new(0, producer);
        self.registers.write_with(
            transaction.0,
            transaction.1,
            |offset, old, next| {
                if offset == ControlRingRegisters::BASE.offset()
                    || offset == ControlRingRegisters::CAPACITY.offset()
                {
                    if domain != Domain::Host {
                        return Err(ControlError::Permission);
                    }
                    if enabled && (old.base != next.base || old.capacity != next.capacity) {
                        return Err(ControlError::ConfigurationLocked);
                    }
                    return Ok(());
                }
                let writing_producer = offset == ControlRingRegisters::PRODUCER.offset();
                if writing_producer != (domain == producer) {
                    return Err(ControlError::Permission);
                }
                if !enabled {
                    return Err(ControlError::Disabled);
                }
                candidate.registers.values = *next;
                if writing_producer {
                    candidate.with_producer(pointers, next.producer.raw())
                } else {
                    candidate.with_consumer(pointers, next.consumer.raw())
                }
                .map_err(ControlError::Ring)?;
                Ok(())
            },
            |_, _| {},
        )
    }
}
