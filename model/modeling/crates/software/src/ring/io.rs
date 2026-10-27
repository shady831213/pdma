use hardware_interface_memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_register::RegisterAccess;

/// Borrowed cursor transport and entry memory for a software ring.
pub trait SoftRingIo: Sync {
    /// Cursor transport failure.
    type Error;
    /// Entry memory backend.
    type Memory: MemoryAccess + Sync;
    /// Entry memory selected by the assembly.
    fn memory(&self) -> &Self::Memory;
    /// Reads the authoritative producer cursor.
    fn read_producer(&self) -> impl Future<Output = Result<u32, Self::Error>> + Send;
    /// Reads the authoritative consumer cursor.
    fn read_consumer(&self) -> impl Future<Output = Result<u32, Self::Error>> + Send;
    /// Publishes the producer cursor.
    fn write_producer(&self, value: u32) -> impl Future<Output = Result<(), Self::Error>> + Send;
    /// Publishes the consumer cursor.
    fn write_consumer(&self, value: u32) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Cursor fields stored as exact four-byte little-endian memory accesses.
#[derive(Debug)]
pub struct MemoryRingIo<'a, M> {
    memory: &'a M,
    producer: MemoryAddress,
    consumer: MemoryAddress,
}

impl<'a, M> MemoryRingIo<'a, M> {
    /// Borrows memory and the two independent cursor addresses.
    pub const fn new(memory: &'a M, producer: MemoryAddress, consumer: MemoryAddress) -> Self {
        Self {
            memory,
            producer,
            consumer,
        }
    }
}

impl<M: MemoryAccess + Sync> SoftRingIo for MemoryRingIo<'_, M> {
    type Error = M::Error;
    type Memory = M;
    fn memory(&self) -> &M {
        self.memory
    }
    async fn read_producer(&self) -> Result<u32, Self::Error> {
        self.memory
            .read_typed(self.producer, MemoryAlignment::ONE)
            .map(u32::from_le_bytes)
    }
    async fn read_consumer(&self) -> Result<u32, Self::Error> {
        self.memory
            .read_typed(self.consumer, MemoryAlignment::ONE)
            .map(u32::from_le_bytes)
    }
    async fn write_producer(&self, value: u32) -> Result<(), Self::Error> {
        self.memory
            .write(self.producer, MemoryAlignment::ONE, &value.to_le_bytes())
    }
    async fn write_consumer(&self, value: u32) -> Result<(), Self::Error> {
        self.memory
            .write(self.consumer, MemoryAlignment::ONE, &value.to_le_bytes())
    }
}

/// Numeric register offsets for independent ring cursors.
#[derive(Clone, Copy, Debug)]
pub struct RingCursorRegisters {
    /// Producer register offset.
    pub producer: u32,
    /// Consumer register offset.
    pub consumer: u32,
}

/// Register-backed cursors sharing the same endpoint as memory-backed cursors.
#[derive(Debug)]
pub struct RegisterRingIo<'a, B, M> {
    bus: &'a B,
    memory: &'a M,
    registers: RingCursorRegisters,
}

impl<'a, B, M> RegisterRingIo<'a, B, M> {
    /// Borrows the register transport and entry memory.
    pub const fn new(bus: &'a B, memory: &'a M, registers: RingCursorRegisters) -> Self {
        Self {
            bus,
            memory,
            registers,
        }
    }
}

impl<B: RegisterAccess + Sync, M: MemoryAccess + Sync> SoftRingIo for RegisterRingIo<'_, B, M> {
    type Error = B::Error;
    type Memory = M;
    fn memory(&self) -> &M {
        self.memory
    }
    async fn read_producer(&self) -> Result<u32, Self::Error> {
        let mut bytes = [0; 4];
        self.bus.read(self.registers.producer, &mut bytes).await?;
        Ok(u32::from_le_bytes(bytes))
    }
    async fn read_consumer(&self) -> Result<u32, Self::Error> {
        let mut bytes = [0; 4];
        self.bus.read(self.registers.consumer, &mut bytes).await?;
        Ok(u32::from_le_bytes(bytes))
    }
    async fn write_producer(&self, value: u32) -> Result<(), Self::Error> {
        self.bus
            .write(self.registers.producer, &value.to_le_bytes())
            .await
    }
    async fn write_consumer(&self, value: u32) -> Result<(), Self::Error> {
        self.bus
            .write(self.registers.consumer, &value.to_le_bytes())
            .await
    }
}
