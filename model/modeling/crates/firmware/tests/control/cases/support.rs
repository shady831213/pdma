pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, NotificationWait, RegisterAccess,
    SharedMemory,
};
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::{
    CONTROL_CHANNEL_BASE, ControlQueue, ControlRegisters, ControlRequest, ControlResponse,
    ControlResponseRaw, ControlRingState,
};
use std::{
    collections::BTreeMap,
    future::Future,
    io,
    pin::Pin,
    sync::Mutex,
    task::{Context, Poll, Waker},
};
use tokio::sync::Notify;

pub(super) const QUEUE: u64 = 0x1000;
pub(super) const COMMAND: u64 = 0x1100;
pub(super) const RESPONSE: u64 = 0x1200;

#[derive(Default)]
pub(super) struct Bus {
    pub(super) registers: Mutex<BTreeMap<u32, Vec<u8>>>,
    pub(super) writes: Mutex<Vec<(u32, Vec<u8>)>>,
    pub(super) notification: Notify,
}

pub(super) fn register(offset: usize) -> TestResult<u32> {
    Ok(CONTROL_CHANNEL_BASE + u32::try_from(offset)?)
}

impl RegisterAccess for &Bus {
    type Error = io::Error;
    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        match self
            .registers
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?
            .get(&address)
        {
            Some(value) => bytes.copy_from_slice(value),
            None => bytes.fill(0),
        }
        Ok(())
    }
    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.registers
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?
            .insert(address, bytes.to_vec());
        self.writes
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?
            .push((address, bytes.to_vec()));
        Ok(())
    }
}

impl NotificationWait for &Bus {
    async fn wait_notification(&self) {
        self.notification.notified().await;
    }
}

pub(super) fn fixture() -> TestResult<(Bus, SharedMemory)> {
    let bus = Bus::default();
    bus.registers
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?
        .insert(
            register(ControlRegisters::ENABLE.offset())?,
            1_u32.to_le_bytes().to_vec(),
        );
    bus.registers
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?
        .insert(
            register(ControlRegisters::BOOTSTRAP.offset())?,
            QUEUE.to_le_bytes().to_vec(),
        );
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(QUEUE),
        0x400,
        MemoryAlignment::ONE,
    )?])?;
    let queue = ControlQueue {
        command: ControlRingState {
            base: COMMAND,
            capacity: 1,
            producer: 0,
            consumer: 0,
        },
        response: ControlRingState {
            base: RESPONSE,
            capacity: 1,
            producer: 0,
            consumer: 0,
        },
    };
    memory.write_typed(
        MemoryAddress::new(QUEUE),
        MemoryAlignment::ONE,
        &queue.encode()?,
    )?;
    Ok((bus, memory))
}

pub(super) fn write_word(memory: &SharedMemory, offset: usize, word: u32) -> TestResult {
    memory.write(
        MemoryAddress::new(QUEUE + u64::try_from(offset)?),
        MemoryAlignment::ONE,
        &word.to_le_bytes(),
    )?;
    Ok(())
}

pub(super) fn read_word(memory: &SharedMemory, offset: usize) -> TestResult<u32> {
    Ok(u32::from_le_bytes(memory.read_typed(
        MemoryAddress::new(QUEUE + u64::try_from(offset)?),
        MemoryAlignment::ONE,
    )?))
}

pub(super) fn publish(memory: &SharedMemory, request: ControlRequest, producer: u32) -> TestResult {
    memory.write_typed(
        MemoryAddress::new(COMMAND),
        MemoryAlignment::ONE,
        &request.encode()?,
    )?;
    write_word(
        memory,
        ControlQueue::COMMAND_OFFSET + ControlRingState::PRODUCER_OFFSET,
        producer,
    )?;
    Ok(())
}

pub(super) fn response(memory: &SharedMemory) -> TestResult<ControlResponse> {
    Ok(memory
        .read_typed::<ControlResponseRaw>(MemoryAddress::new(RESPONSE), MemoryAlignment::ONE)?
        .decode()?)
}

pub(super) fn poll<F: Future>(program: Pin<&mut F>) -> Poll<F::Output> {
    program.poll(&mut Context::from_waker(Waker::noop()))
}
