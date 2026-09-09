use hardware_interface_runtime::{
    MemoryAddress, RegisterAccess, SharedMemory, SharedMemoryAllocator,
};
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
    DmaAddress, RingLayout,
};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, MmioRegisterEndpoint, MmioRegisterError,
};
use std::sync::Arc;

pub(in crate::mmio) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(in crate::mmio) fn device(
    config: ResourceConfig,
) -> (AsyncDevice, AsyncDeviceWorker, SharedMemoryAllocator) {
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (device, worker) = AsyncDevice::new(allocator.memory_access(), config);
    (device, worker, allocator)
}

pub(in crate::mmio) async fn write_u32(
    endpoint: &MmioRegisterEndpoint,
    offset: u32,
    value: u32,
) -> Result<(), MmioRegisterError> {
    endpoint.write(offset, &value.to_le_bytes()).await
}

pub(in crate::mmio) async fn write_u64(
    endpoint: &MmioRegisterEndpoint,
    offset: u32,
    value: u64,
) -> Result<(), MmioRegisterError> {
    endpoint.write(offset, &value.to_le_bytes()).await
}

pub(in crate::mmio) async fn read_u32(
    endpoint: &MmioRegisterEndpoint,
    offset: u32,
) -> TestResult<u32> {
    let mut bytes = [0; 4];
    endpoint.read(offset, &mut bytes).await?;
    Ok(u32::from_le_bytes(bytes))
}

pub(in crate::mmio) async fn read_u64(
    endpoint: &MmioRegisterEndpoint,
    offset: u32,
) -> TestResult<u64> {
    let mut bytes = [0; 8];
    endpoint.read(offset, &mut bytes).await?;
    Ok(u64::from_le_bytes(bytes))
}

pub(in crate::mmio) fn q(offset: usize) -> TestResult<u32> {
    Ok(queue_offset(0, offset)?)
}

pub(in crate::mmio) async fn program_activation(
    endpoint: &MmioRegisterEndpoint,
    regions: &[DmaAddress],
    depth: u32,
) -> TestResult {
    let [submission, completion, event] = regions else {
        return Err("three queue regions required".into());
    };
    for (offset, value) in [
        (QueueRegisters::SQ_BASE.offset(), submission.raw()),
        (QueueRegisters::CQ_BASE.offset(), completion.raw()),
        (QueueRegisters::EQ_BASE.offset(), event.raw()),
    ] {
        write_u64(endpoint, q(offset)?, value).await?;
    }
    for offset in [
        QueueRegisters::SQ_DEPTH.offset(),
        QueueRegisters::CQ_DEPTH.offset(),
        QueueRegisters::EQ_DEPTH.offset(),
    ] {
        write_u32(endpoint, q(offset)?, depth).await?;
    }
    Ok(())
}

pub(in crate::mmio) async fn create_queue(
    handle: &AsyncDeviceHandle,
    allocator: &SharedMemoryAllocator,
    depth: u32,
) -> TestResult<[DmaAddress; 3]> {
    let alignment = hardware_interface_runtime::MemoryAlignment::try_new(64)?;
    let regions = [
        RingLayout::<AbiCommand>::checked_extent(u64::from(depth))?,
        RingLayout::<AbiCompletion>::checked_extent(u64::from(depth))?,
        RingLayout::<AbiEvent>::checked_extent(u64::from(depth))?,
    ]
    .map(|length| {
        allocator
            .allocate(length, alignment)
            .map(|address| DmaAddress::new(address.raw()))
    });
    let [submission, completion, event] = regions;
    let regions = [submission?, completion?, event?];
    let endpoint = handle.register_endpoint();
    program_activation(&endpoint, &regions, depth).await?;
    write_u32(&endpoint, q(QueueRegisters::ENABLE.offset())?, 1).await?;
    Ok(regions)
}

pub(in crate::mmio) async fn submit_copy(
    handle: &AsyncDeviceHandle,
    allocator: &SharedMemoryAllocator,
    (submission, slot): (DmaAddress, u64),
) -> TestResult<DmaAddress> {
    let source = DmaAddress::new(
        allocator
            .allocate(4, hardware_interface_runtime::MemoryAlignment::try_new(64)?)?
            .raw(),
    );
    let destination = DmaAddress::new(
        allocator
            .allocate(4, hardware_interface_runtime::MemoryAlignment::try_new(64)?)?
            .raw(),
    );
    handle.write_memory(source, &[1, 2, 3, 4]).await?;
    let descriptor = AbiCommand::from_payload(
        slot,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: source.raw(),
            destination_dma_address: destination.raw(),
            length: 4.into(),
        }),
    );
    let mut bytes = [0; AbiCommand::RECORD_SIZE];
    descriptor.encode(&mut bytes)?;
    handle
        .write_memory(
            submission.checked_add(slot * u64::try_from(AbiCommand::RECORD_SIZE)?)?,
            &bytes,
        )
        .await?;
    write_u64(
        &handle.register_endpoint(),
        q(QueueRegisters::SQ_PRODUCER.offset())?,
        slot + 1,
    )
    .await?;
    Ok(destination)
}

pub(in crate::mmio) async fn cursors(endpoint: &MmioRegisterEndpoint) -> TestResult<[u64; 7]> {
    let mut values = [0; 7];
    for (value, offset) in values.iter_mut().zip([
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::SQ_CAPTURED.offset(),
        QueueRegisters::SQ_CONSUMER.offset(),
        QueueRegisters::CQ_PRODUCER.offset(),
        QueueRegisters::CQ_CONSUMER.offset(),
        QueueRegisters::EQ_PRODUCER.offset(),
        QueueRegisters::EQ_CONSUMER.offset(),
    ]) {
        *value = read_u64(endpoint, q(offset)?).await?;
    }
    Ok(values)
}
