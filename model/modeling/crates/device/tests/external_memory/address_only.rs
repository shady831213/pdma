use super::{Arc, AsyncDevice, ExternalBytes, Mutex, ResourceConfig};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadStridedCopyValues,
    AbiCommandPayloadValues, AbiCompletion, AbiEvent, AbiEventAccess, AbiEventKind, AbiEventStage,
    registers::{QueueRegisters, queue_offset},
};
use pdma_model_device::MmioRegisterEndpoint;
use std::{future::Future, io, task::Poll};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn interior_rings_execute_copy_without_device_memory_registration() -> TestResult {
    // Given: one external byte extent contains interior rings and payloads without registration.
    let external = Arc::new(ExternalBytes(Mutex::new([0; 1024])));
    let descriptor = AbiCommand::from_payload(
        42,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: 0x81c0,
            destination_dma_address: 0x8200,
            length: 4.into(),
        }),
    );
    let mut command = [0; 64];
    descriptor.encode(&mut command)?;
    external.write(MemoryAddress::new(0x8040), MemoryAlignment::ONE, &command)?;
    external.write(
        MemoryAddress::new(0x81c0),
        MemoryAlignment::ONE,
        &[3, 5, 7, 11],
    )?;
    external.write(MemoryAddress::new(0x8200), MemoryAlignment::ONE, &[0x55; 4])?;
    let (device, worker) = AsyncDevice::new(external.clone(), ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();

    // When: raw MMIO enables the interior rings and the real worker executes their command.
    activate(&endpoint).await?;
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

    // Then: addressed payload and completion bytes publish with no mapping registration.
    assert_eq!((summary.steps, summary.errors), (1, 0));
    let mut destination = [0; 4];
    external.read(
        MemoryAddress::new(0x8200),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [3, 5, 7, 11]);
    let mut completion = [0; 32];
    external.read(
        MemoryAddress::new(0x80c0),
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
    Ok(())
}

#[tokio::test]
async fn unavailable_second_destination_preserves_first_row_before_any_write() -> TestResult {
    // Given: two strided rows place the second destination just outside the external extent.
    let external = Arc::new(ExternalBytes(Mutex::new([0; 1024])));
    let descriptor = AbiCommand::from_payload(
        43,
        AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
            source_dma_address: 0x81c0,
            destination_dma_address: 0x83fc,
            rows: 2.into(),
            row_bytes: 4.into(),
            source_stride: 4.into(),
            destination_stride: 4.into(),
        }),
    );
    let mut command = [0; 64];
    descriptor.encode(&mut command)?;
    external.write(MemoryAddress::new(0x8040), MemoryAlignment::ONE, &command)?;
    external.write(
        MemoryAddress::new(0x81c0),
        MemoryAlignment::ONE,
        &[1, 2, 3, 4, 5, 6, 7, 8],
    )?;
    external.write(MemoryAddress::new(0x83fc), MemoryAlignment::ONE, &[0x55; 4])?;
    let (device, worker) = AsyncDevice::new(external.clone(), ResourceConfig::default());
    let handle = device.handle();
    let endpoint = handle.register_endpoint();

    // When: the worker prevalidates both rows against the actual backend extent.
    activate(&endpoint).await?;
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

    // Then: no earlier row is written and one exact destination fault retires without success.
    assert_eq!((summary.steps, summary.errors), (0, 1));
    let mut destination = [0; 4];
    external.read(
        MemoryAddress::new(0x83fc),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [0x55; 4]);
    let mut completion = [0; 32];
    external.read(
        MemoryAddress::new(0x80c0),
        MemoryAlignment::ONE,
        &mut completion,
    )?;
    assert_eq!(completion, [0; 32]);
    let mut event_bytes = [0; 64];
    external.read(
        MemoryAddress::new(0x8100),
        MemoryAlignment::ONE,
        &mut event_bytes,
    )?;
    let event = AbiEvent::decode(&event_bytes)?;
    assert_eq!(event.command_id, 43);
    assert_eq!(
        event.fault().tuple(),
        (
            AbiEventKind::UnmappedDmaAddress,
            0x8400,
            4,
            0,
            AbiEventStage::Destination.raw(),
            AbiEventAccess::Write.raw(),
            1,
        )
    );
    assert_eq!(
        cursor(&endpoint, QueueRegisters::SQ_CONSUMER.offset()).await?,
        1
    );
    assert_eq!(
        cursor(&endpoint, QueueRegisters::CQ_PRODUCER.offset()).await?,
        0
    );
    assert_eq!(
        cursor(&endpoint, QueueRegisters::EQ_PRODUCER.offset()).await?,
        1
    );
    Ok(())
}

async fn activate(endpoint: &MmioRegisterEndpoint) -> TestResult {
    for (local, base) in [
        (QueueRegisters::SQ_BASE.offset(), 0x8040_u64),
        (QueueRegisters::CQ_BASE.offset(), 0x80c0),
        (QueueRegisters::EQ_BASE.offset(), 0x8100),
    ] {
        endpoint
            .write(queue_offset(0, local)?, &base.to_le_bytes())
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
    let activation = endpoint
        .write(
            queue_offset(0, QueueRegisters::ENABLE.offset())?,
            &1_u32.to_le_bytes(),
        )
        .await;
    assert_eq!(
        activation,
        Ok(()),
        "ENABLE accepts rings inside the external extent"
    );
    Ok(())
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
                "address-only worker exceeded poll budget",
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
