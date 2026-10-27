use pdma_model_contract::hw_components::control::ControlConfiguration;
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
use hardware_interface_irq::{InterruptEmitter, PendingInterrupt, pending_interrupt};
use hardware_interface_memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use hardware_interface_register::RegisterAccess;
use hardware_interface_runtime::{CpuAccessError, InterruptRuntime, SoftwareCpu};
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlResponse, ControlResponseRaw, ControlRingState,
};
use pdma_model_contract::dma::{DmaDiscovery, DmaRegisters};
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_software::control::{ControlIrq, ControlIsr, ManagementAddresses};
use std::{
    collections::BTreeMap,
    future::Future,
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

pub(super) const QUEUE: u64 = 0x1000;
pub(super) const COMMAND: u64 = 0x1100;
pub(super) const RESPONSE: u64 = 0x1200;
pub(super) const ADDRESSES: ManagementAddresses = ManagementAddresses {
    dma: 0,
    windows: 0x2000,
    control: 0x3000,
    firmware: 0x4000,
    contexts: None,
};

pub(super) struct Bus {
    pub(super) registers: Mutex<BTreeMap<u32, Vec<u8>>>,
    pub(super) writes: Mutex<Vec<(u32, Vec<u8>)>>,
    pub(super) notification: InterruptEmitter,
    source: PendingInterrupt<ControlIrq>,
}

impl Default for Bus {
    fn default() -> Self {
        let (notification, source) = pending_interrupt(ControlIrq);
        Self {
            registers: Mutex::default(),
            writes: Mutex::default(),
            notification,
            source,
        }
    }
}

pub(super) async fn run(
    bus: &Bus,
    memory: SharedMemory,
) -> Result<std::convert::Infallible, pdma_model_firmware::Error<CpuAccessError<io::Error>>> {
    run_at(bus, memory, ADDRESSES).await
}

pub(super) async fn run_at(
    bus: &Bus,
    memory: SharedMemory,
    addresses: ManagementAddresses,
) -> Result<std::convert::Infallible, pdma_model_firmware::Error<CpuAccessError<io::Error>>> {
    let cpu = Arc::new(SoftwareCpu::new());
    let bound = cpu.bind_registers(bus);
    let runtime = InterruptRuntime::new(cpu, bus.source.clone());
    let (result, _) = runtime
        .run_main::<ControlIsr, _>(async |registration| {
            pdma_model_firmware::main(bound, memory, registration, addresses).await
        })
        .await
        .map_err(|error| {
            pdma_model_firmware::Error::Bus(CpuAccessError::Access(io::Error::other(error)))
        })?;
    result
}

pub(super) fn register(offset: usize) -> TestResult<u32> {
    Ok(ADDRESSES.control + u32::try_from(offset)?)
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

pub(super) fn fixture() -> TestResult<(Bus, SharedMemory)> {
    let bus = Bus::default();
    bus.registers
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?
        .insert(
            ADDRESSES.dma
                + u32::try_from(
                    DmaRegisters::DISCOVERY.offset() + DmaDiscovery::CHANNEL_NUM.offset(),
                )?,
            1_u32.to_le_bytes().to_vec(),
        );
    bus.registers
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?
        .insert(
            register(ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset())?,
            1_u32.to_le_bytes().to_vec(),
        );
    bus.registers
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?
        .insert(
            register(ControlRegisters::CONFIG.offset() + ControlConfiguration::BOOTSTRAP.offset())?,
            QUEUE.to_le_bytes().to_vec(),
        );
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(QUEUE),
        0x400,
        MemoryAlignment::ONE,
    )?])?;
    let queue = ControlQueue {
        event: ControlRingState {
            base: 0x1300,
            capacity: 1,
            producer: 0,
            consumer: 0,
        },
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
