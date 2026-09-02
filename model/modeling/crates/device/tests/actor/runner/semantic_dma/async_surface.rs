use super::TestResult;
use pdma_model_contract::semantic::{
    BufferId, Command, CommandId, CompletionStatus, DmaRangeStart, EventErrorKind, QueueConfig,
    QueueConfigHandle, SoftwareOwnerId, desc::CopyRequest, queue::EventRouting,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, AsyncRunSummary};

struct Fixture {
    runner: AsyncDevice,
    worker: tokio::task::JoinHandle<AsyncRunSummary>,
    handle: AsyncDeviceHandle,
    queue: QueueConfigHandle,
    source: BufferId,
    destination: BufferId,
    source_end: pdma_model_contract::abi::DmaAddress,
    expected: Vec<u8>,
}

pub(super) async fn exercise() -> TestResult {
    let fixture = Fixture::new().await?;
    fixture.copy_nonzero_range().await?;
    let errors = fixture.mapped_fault(0).await?;
    let errors = fixture.unknown_fault(errors).await?;
    fixture.overflow_fault(errors).await?;
    fixture.runner.request_shutdown().await;
    fixture.worker.await?;
    Ok(())
}

impl Fixture {
    async fn new() -> TestResult<Self> {
        let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
        ));
        let worker = tokio::spawn(worker.run());
        let handle = runner.handle();
        let queue = handle
            .create_queue(QueueConfig::new(
                8,
                8,
                EventRouting::QueueLocal,
                SoftwareOwnerId::new(0),
            ))
            .await?;
        let source = handle.allocate_buffer(8).await?;
        let destination = handle.allocate_buffer(8).await?;
        let source_region = handle
            .region_for_object(source)
            .await
            .ok_or_else(|| std::io::Error::other("source mapping missing"))?;
        handle
            .write_buffer(source, 0, &[10, 11, 12, 13, 14, 15, 16, 17])
            .await?;
        handle.write_buffer(destination, 0, &[90; 8]).await?;
        Ok(Self {
            runner,
            worker,
            handle,
            queue,
            source,
            destination,
            source_end: source_region.exclusive_end(),
            expected: vec![90, 90, 90, 12, 13, 14, 90, 90],
        })
    }

    async fn copy_nonzero_range(&self) -> TestResult {
        let request = CopyRequest::with_starts(
            DmaRangeStart::new(self.source, 2),
            DmaRangeStart::new(self.destination, 3),
            3,
        );
        self.submit(CommandId::new(0), request).await?;
        self.runner.wait_for_progress_after(0).await;
        let completion = self
            .handle
            .poll_completion(self.queue)
            .await?
            .ok_or_else(|| std::io::Error::other("copy completion missing"))?;
        assert_eq!(completion.command_id(), CommandId::new(0));
        assert_eq!(completion.status(), CompletionStatus::Success);
        self.assert_destination_unchanged().await?;
        eprintln!(
            "async nonzero copy destination={:?}",
            self.handle.read_buffer(self.destination, 0, 8).await?
        );
        Ok(())
    }

    async fn mapped_fault(&self, errors: usize) -> TestResult<usize> {
        let request = CopyRequest::with_starts(
            DmaRangeStart::new(self.source, 7),
            DmaRangeStart::new(self.destination, 0),
            2,
        );
        self.submit(CommandId::new(1), request).await?;
        let snapshot = self.runner.wait_for_error_after(errors).await;
        let event = self.next_event().await?;
        assert_eq!(
            event.error_kind(),
            EventErrorKind::OutOfBounds { len: 8, end: 9 }
        );
        assert_eq!(event.buffer(), self.source);
        assert_eq!(
            event.fault_address(),
            Some(pdma_model_contract::semantic::FaultAddress::new(
                self.source_end.raw()
            ))
        );
        self.assert_no_success_or_bytes().await?;
        eprintln!("mapped fault={event:?}");
        Ok(snapshot.errors)
    }

    async fn unknown_fault(&self, errors: usize) -> TestResult<usize> {
        let request = CopyRequest::with_starts(
            DmaRangeStart::new(BufferId::new(99), 1),
            DmaRangeStart::new(self.destination, 0),
            2,
        );
        self.submit(CommandId::new(2), request).await?;
        let snapshot = self.runner.wait_for_error_after(errors).await;
        let event = self.next_event().await?;
        assert_eq!(event.error_kind(), EventErrorKind::UnknownBuffer);
        assert_eq!(event.buffer(), BufferId::new(99));
        assert_eq!(event.fault_address(), None);
        self.assert_no_success_or_bytes().await?;
        eprintln!("unknown-object fault={event:?}");
        Ok(snapshot.errors)
    }

    async fn overflow_fault(&self, errors: usize) -> TestResult {
        let request = CopyRequest::with_starts(
            DmaRangeStart::new(self.source, usize::MAX),
            DmaRangeStart::new(self.destination, 0),
            2,
        );
        self.submit(CommandId::new(3), request).await?;
        self.runner.wait_for_error_after(errors).await;
        let event = self.next_event().await?;
        assert_eq!(event.error_kind(), EventErrorKind::RangeOverflow);
        assert_eq!(event.buffer(), self.source);
        assert_eq!(event.fault_address(), None);
        self.assert_no_success_or_bytes().await?;
        eprintln!("range-overflow fault={event:?}");
        Ok(())
    }

    async fn submit(&self, command_id: CommandId, request: CopyRequest) -> TestResult {
        let command = Command::new(
            command_id,
            pdma_model_contract::semantic::CommandOperation::Copy(request),
        );
        self.handle.submit_command(self.queue, command).await?;
        self.handle.ring_doorbell(self.queue).await?;
        Ok(())
    }

    async fn next_event(&self) -> TestResult<pdma_model_contract::semantic::EventRecord> {
        self.handle
            .poll_queue_event(self.queue)
            .await?
            .ok_or_else(|| std::io::Error::other("fault event missing").into())
    }

    async fn assert_no_success_or_bytes(&self) -> TestResult {
        assert_eq!(self.handle.poll_completion(self.queue).await?, None);
        self.assert_destination_unchanged().await
    }

    async fn assert_destination_unchanged(&self) -> TestResult {
        assert_eq!(
            self.handle.read_buffer(self.destination, 0, 8).await?,
            self.expected
        );
        Ok(())
    }
}
