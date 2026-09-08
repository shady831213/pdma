use super::*;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion,
    mmio::{MmioResult, QueueCommand},
};
use pdma_model_device::MmioRegisterEndpoint;
use std::{future::Future, io, task::Poll};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn raw_mmio_executes_abi_rings_in_externally_provisioned_memory() -> TestResult {
    // Given externally owned SQ/CQ/EQ and payload bytes with only address read/write access.
    let external = provisioned_bytes()?;
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::external(external.clone().into()),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let regions = [
        (0x8000, 128),
        (0x8080, 64),
        (0x80c0, 128),
        (0x8140, 4),
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

    // When raw activation, producer publication and doorbell capture drive the actual worker.
    for (offset, value) in [
        (0x100, 0_u64),
        (0x108, 0),
        (0x130, 7),
        (0x138, 0x8000),
        (0x148, 0x8080),
        (0x158, 0x80c0),
    ] {
        endpoint.write(offset, &value.to_le_bytes()).await?;
    }
    for offset in [0x120, 0x124, 0x140, 0x150, 0x160] {
        endpoint.write(offset, &2_u32.to_le_bytes()).await?;
    }
    issue(&endpoint, QueueCommand::Activate).await?;
    endpoint.write(0x110, &0_u64.to_le_bytes()).await?;
    endpoint.write(0x118, &1_u64.to_le_bytes()).await?;
    issue(&endpoint, QueueCommand::PublishSqProducer).await?;
    issue(&endpoint, QueueCommand::CaptureSqProducer).await?;
    let (summary, progress) = bounded(async {
        tokio::join!(worker.run(), async {
            let progress = bounded(handle.wait_for_progress_after(0)).await;
            handle.request_shutdown().await;
            progress
        })
    })
    .await?;
    progress?;

    // Then external payload and CQ bytes expose the copy and its exact successful completion.
    assert_eq!((summary.steps, summary.errors), (1, 0));
    let mut destination = [0; 4];
    external.read(
        MemoryAddress::new(0x8180),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [3, 5, 7, 11]);
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
    issue(&endpoint, QueueCommand::ReadStatus).await?;
    for (offset, expected) in [(0x1a8, 1_u64), (0x1b0, 1), (0x1c0, 0)] {
        let mut bytes = [0; 8];
        endpoint.read(offset, &mut bytes).await?;
        assert_eq!(u64::from_le_bytes(bytes), expected);
    }
    assert_eq!(handle.memory_generation().await, 1);
    Ok(())
}

fn provisioned_bytes() -> TestResult<Arc<ExternalBytes<512>>> {
    let external = Arc::new(ExternalBytes(Mutex::new([0; 512])));
    let descriptor = AbiCommand::from_payload(
        42,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: 0x8140,
            destination_dma_address: 0x8180,
            length: 4.into(),
        }),
    );
    let mut command_bytes = [0; 64];
    descriptor.encode(&mut command_bytes)?;
    external.write(
        MemoryAddress::new(0x8000),
        MemoryAlignment::ONE,
        &command_bytes,
    )?;
    external.write(
        MemoryAddress::new(0x8140),
        MemoryAlignment::ONE,
        &[3, 5, 7, 11],
    )?;
    external.write(MemoryAddress::new(0x8180), MemoryAlignment::ONE, &[0x55; 4])?;
    Ok(external)
}

async fn issue(endpoint: &MmioRegisterEndpoint, command: QueueCommand) -> TestResult {
    endpoint.write(0x164, &command.code().to_le_bytes()).await?;
    let mut result = [0; 4];
    endpoint.read(0x168, &mut result).await?;
    assert_eq!(u32::from_le_bytes(result), MmioResult::Ok.code());
    Ok(())
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
