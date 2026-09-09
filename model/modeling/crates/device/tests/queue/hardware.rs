use hardware_interface_runtime::{
    MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory, SharedMemoryAllocator,
};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};
use pdma_model_device::{AsyncDeviceHandle, MmioRegisterEndpoint};
use std::sync::Arc;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) fn allocator() -> SharedMemoryAllocator {
    SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000))
}

#[derive(Clone, Copy)]
pub(super) struct Allocation {
    base: DmaAddress,
    length: u64,
}

impl Allocation {
    pub(super) const fn base(self) -> DmaAddress {
        self.base
    }
    pub(super) const fn length(self) -> u64 {
        self.length
    }
}

pub(super) fn allocate(memory: &SharedMemoryAllocator, length: u64) -> TestResult<Allocation> {
    let base = memory.allocate(length, MemoryAlignment::try_new(64)?)?;
    Ok(Allocation {
        base: DmaAddress::new(base.raw()),
        length,
    })
}

pub(super) fn rings(
    memory: &SharedMemoryAllocator,
    depths: [u64; 3],
) -> TestResult<[DmaAddress; 3]> {
    Ok([
        allocate(memory, depths[0] * 64)?.base(),
        allocate(memory, depths[1] * 32)?.base(),
        allocate(memory, depths[2] * 64)?.base(),
    ])
}

pub(super) async fn write(endpoint: &MmioRegisterEndpoint, offset: u32, value: u64) -> TestResult {
    let register =
        pdma_model_contract::abi::registers::locate(offset).ok_or("register location")?;
    let width = match register {
        pdma_model_contract::abi::registers::RegisterLocation::Global { register } => {
            register.width()
        }
        pdma_model_contract::abi::registers::RegisterLocation::Queue { register, .. } => {
            register.width()
        }
        pdma_model_contract::abi::registers::RegisterLocation::Irq { register, .. } => {
            register.width()
        }
    };
    endpoint
        .write(
            offset,
            value.to_le_bytes().get(..width).ok_or("register width")?,
        )
        .await?;
    Ok(())
}

pub(super) async fn read(endpoint: &MmioRegisterEndpoint, offset: u32) -> TestResult<u64> {
    let register =
        pdma_model_contract::abi::registers::locate(offset).ok_or("register location")?;
    let width = match register {
        pdma_model_contract::abi::registers::RegisterLocation::Global { register } => {
            register.width()
        }
        pdma_model_contract::abi::registers::RegisterLocation::Queue { register, .. } => {
            register.width()
        }
        pdma_model_contract::abi::registers::RegisterLocation::Irq { register, .. } => {
            register.width()
        }
    };
    let mut bytes = [0; 8];
    endpoint
        .read(offset, bytes.get_mut(..width).ok_or("register width")?)
        .await?;
    Ok(u64::from_le_bytes(bytes))
}

pub(super) async fn configure(
    endpoint: &MmioRegisterEndpoint,
    index: u32,
    rings: [(DmaAddress, u64); 3],
) -> TestResult {
    for ((base, depth), (base_register, depth_register)) in rings.into_iter().zip([
        (
            QueueRegisters::SQ_BASE.offset(),
            QueueRegisters::SQ_DEPTH.offset(),
        ),
        (
            QueueRegisters::CQ_BASE.offset(),
            QueueRegisters::CQ_DEPTH.offset(),
        ),
        (
            QueueRegisters::EQ_BASE.offset(),
            QueueRegisters::EQ_DEPTH.offset(),
        ),
    ]) {
        write(endpoint, queue_offset(index, base_register)?, base.raw()).await?;
        write(endpoint, queue_offset(index, depth_register)?, depth).await?;
    }
    Ok(())
}

pub(super) async fn activate(
    handle: &AsyncDeviceHandle,
    memory: &SharedMemoryAllocator,
    index: u32,
    depths: [u64; 3],
) -> TestResult<[DmaAddress; 3]> {
    let addresses = rings(memory, depths)?;
    let endpoint = handle.register_endpoint();
    configure(
        &endpoint,
        index,
        [
            (addresses[0], depths[0]),
            (addresses[1], depths[1]),
            (addresses[2], depths[2]),
        ],
    )
    .await?;
    write(
        &endpoint,
        queue_offset(index, QueueRegisters::ENABLE.offset())?,
        1,
    )
    .await?;
    Ok(addresses)
}

pub(super) async fn idle(handle: &AsyncDeviceHandle) {
    let mut snapshot = handle.snapshot().await;
    while snapshot.last_status != pdma_model_device::AsyncStepStatus::Idle {
        snapshot = handle.wait_for_change_after(snapshot).await;
    }
}
