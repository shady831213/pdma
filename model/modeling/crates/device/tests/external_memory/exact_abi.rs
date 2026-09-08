use super::*;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
    AbiEventAccess, AbiEventStage,
    registers::{QueueRegisters, queue_offset},
};
use pdma_model_device::{AsyncDeviceHandle, MmioRegisterEndpoint};
use std::{future::Future, io, task::Poll};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn raw_registers_execute_abi_rings_in_externally_provisioned_memory() -> TestResult {
    // Given externally owned rings and payload bytes with only address read/write access.
    let external = provisioned_bytes(0x8140)?;
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::external(external.clone().into()),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    prepare(&handle, 0x8140).await?;
    let endpoint = handle.register_endpoint();

    // When direct producer and doorbell writes drive the actual worker.
    publish(&endpoint).await?;
    let (summary, progress) = bounded(async {
        tokio::join!(worker.run(), async {
            let progress = bounded(handle.wait_for_progress_after(0)).await;
            handle.request_shutdown().await;
            progress
        })
    })
    .await?;
    progress?;

    // Then both external and host memory access observe exact payload and CQ bytes.
    assert_eq!((summary.steps, summary.errors), (1, 0));
    let mut destination = [0; 4];
    external.read(
        MemoryAddress::new(0x8180),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [3, 5, 7, 11]);
    let mut host_bytes = [0; 4];
    handle.memory_access().read(
        MemoryAddress::new(0x8180),
        MemoryAlignment::ONE,
        &mut host_bytes,
    )?;
    assert_eq!(host_bytes, destination);
    let mut completion = [0; 32];
    external.read(
        MemoryAddress::new(0x8080),
        MemoryAlignment::ONE,
        &mut completion,
    )?;
    assert_eq!(
        AbiCompletion::decode(&completion)?,
        AbiCompletion::success(0, 42)
    );
    assert_eq!(
        cursor(&endpoint, QueueRegisters::SQ_CONSUMER.offset()).await?,
        1
    );
    assert_eq!(
        cursor(&endpoint, QueueRegisters::CQ_PRODUCER.offset()).await?,
        1
    );
    assert_eq!(
        cursor(&endpoint, QueueRegisters::EQ_PRODUCER.offset()).await?,
        0
    );
    assert_eq!(handle.memory_generation().await, 1);
    Ok(())
}

#[tokio::test]
async fn backend_read_fault_publishes_event_without_success_completion() -> TestResult {
    // Given a valid logical source mapping whose external bytes are unavailable.
    let external = provisioned_bytes(0x9000)?;
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::external(external.clone().into()),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    prepare(&handle, 0x9000).await?;
    let endpoint = handle.register_endpoint();

    // When the worker accesses the unavailable source through the exact descriptor.
    publish(&endpoint).await?;
    let (summary, fault) = bounded(async {
        tokio::join!(worker.run(), async {
            let fault = bounded(handle.wait_for_error_after(0)).await;
            handle.request_shutdown().await;
            fault
        })
    })
    .await?;
    fault?;

    // Then the EQ identifies the source fault while payload and CQ remain unchanged.
    assert_eq!((summary.steps, summary.errors), (0, 1));
    assert_eq!(
        cursor(&endpoint, QueueRegisters::CQ_PRODUCER.offset()).await?,
        0
    );
    assert_eq!(
        cursor(&endpoint, QueueRegisters::EQ_PRODUCER.offset()).await?,
        1
    );
    let mut destination = [0; 4];
    external.read(
        MemoryAddress::new(0x8180),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [0x55; 4]);
    let mut cq = [0; 32];
    external.read(MemoryAddress::new(0x8080), MemoryAlignment::ONE, &mut cq)?;
    assert_eq!(cq, [0; 32]);
    let mut eq = [0; 64];
    external.read(MemoryAddress::new(0x80c0), MemoryAlignment::ONE, &mut eq)?;
    let event = AbiEvent::decode(&eq)?;
    assert_eq!(event.command_id, 42);
    assert_eq!(event.fault_locator, 0x9000);
    assert_eq!(
        (event.stage, event.access),
        (AbiEventStage::Source, AbiEventAccess::Read)
    );
    Ok(())
}

async fn prepare(handle: &AsyncDeviceHandle, source: u64) -> TestResult {
    let regions = [
        (0x8000, 128),
        (0x8080, 64),
        (0x80c0, 128),
        (source, 4),
        (0x8180, 4),
    ]
    .into_iter()
    .enumerate()
    .map(|(object, (base, length))| {
        MappedDmaRegion::try_new(BufferId::new(object), DmaAddress::new(base), length)
    })
    .collect::<Result<Vec<_>, _>>()?;
    handle.register_memory_regions(&regions).await?;
    let endpoint = handle.register_endpoint();
    for (local, value) in [
        (QueueRegisters::SQ_BASE.offset(), 0x8000_u64),
        (QueueRegisters::CQ_BASE.offset(), 0x8080),
        (QueueRegisters::EQ_BASE.offset(), 0x80c0),
    ] {
        endpoint
            .write(queue_offset(0, local)?, &value.to_le_bytes())
            .await?;
    }
    for local in [
        QueueRegisters::SQ_DEPTH.offset(),
        QueueRegisters::CQ_DEPTH.offset(),
        QueueRegisters::EQ_DEPTH.offset(),
    ] {
        endpoint
            .write(queue_offset(0, local)?, &2_u32.to_le_bytes())
            .await?;
    }
    endpoint
        .write(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await?;
    Ok(())
}

fn provisioned_bytes(source: u64) -> TestResult<Arc<ExternalBytes<512>>> {
    let external = Arc::new(ExternalBytes(Mutex::new([0; 512])));
    let descriptor = AbiCommand::from_payload(
        42,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: source,
            destination_dma_address: 0x8180,
            length: 4.into(),
        }),
    );
    let mut bytes = [0; 64];
    descriptor.encode(&mut bytes)?;
    external.write(MemoryAddress::new(0x8000), MemoryAlignment::ONE, &bytes)?;
    external.write(
        MemoryAddress::new(0x8140),
        MemoryAlignment::ONE,
        &[3, 5, 7, 11],
    )?;
    external.write(MemoryAddress::new(0x8180), MemoryAlignment::ONE, &[0x55; 4])?;
    Ok(external)
}

async fn publish(endpoint: &MmioRegisterEndpoint) -> TestResult {
    for local in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::DOORBELL.offset(),
    ] {
        endpoint
            .write(queue_offset(0, local)?, &1_u64.to_le_bytes())
            .await?;
    }
    Ok(())
}

async fn cursor(endpoint: &MmioRegisterEndpoint, local: usize) -> TestResult<u64> {
    let mut bytes = [0; 8];
    endpoint.read(queue_offset(0, local)?, &mut bytes).await?;
    Ok(u64::from_le_bytes(bytes))
}

async fn bounded<T>(future: impl Future<Output = T>) -> Result<T, io::Error> {
    let mut future = std::pin::pin!(future);
    let mut polls = 0;
    std::future::poll_fn(|context| {
        polls += 1;
        if polls > 512 {
            return Poll::Ready(Err(io::Error::other(
                "external ABI worker exceeded poll budget",
            )));
        }
        match future.as_mut().poll(context) {
            Poll::Ready(output) => Poll::Ready(Ok(output)),
            Poll::Pending => {
                context.waker().wake_by_ref();
                Poll::Pending
            }
        }
    })
    .await
}
