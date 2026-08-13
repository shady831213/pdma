use pdma_model_contract::{BufferId, CommandId, CopyRequest, TraceEventKind};
use pdma_model_device::{AsyncStepStatus, SyncDevice};
use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;
use pdma_model_host::{AsyncDriver, SoftwareCpu};

struct PollFixture {
    owner: SoftwareCpu,
    driver: AsyncDriver,
    device: SyncDevice,
    retry: CopyRequest,
}

impl PollFixture {
    fn trace_count(&self, kind: TraceEventKind) -> usize {
        self.device
            .trace_events()
            .iter()
            .filter(|event| event.kind() == kind)
            .count()
    }

    async fn retry_completion(
        &mut self,
    ) -> ModelResult<(Option<CommandId>, ModelResult<CommandId>)> {
        self.owner
            .run_main(async {
                let completion = self
                    .driver
                    .poll_completion_on(&mut self.device)
                    .await?
                    .map(pdma_model_contract::CompletionRecord::command_id);
                let submitted = self
                    .driver
                    .submit_copy_on(&mut self.device, self.retry)
                    .await;
                Ok((completion, submitted))
            })
            .await
    }

    async fn retry_event(&mut self) -> ModelResult<(Option<CommandId>, ModelResult<CommandId>)> {
        self.owner
            .run_main(async {
                let event = self
                    .driver
                    .poll_event_on(&mut self.device)
                    .await?
                    .map(pdma_model_contract::EventRecord::command_id);
                let submitted = self
                    .driver
                    .submit_copy_on(&mut self.device, self.retry)
                    .await;
                Ok((event, submitted))
            })
            .await
    }
}

async fn completion_fixture() -> ModelResult<PollFixture> {
    let owner = SoftwareCpu::new();
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    device.enable_tracing();
    let (driver, retry) = owner
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut device, 1, 1).await?;
            let src = driver.allocate_buffer_on(&mut device, 4).await?;
            let dst = driver.allocate_buffer_on(&mut device, 4).await?;
            let retry = CopyRequest::new(src, dst, 4);
            driver.submit_copy_on(&mut device, retry).await?;
            Ok::<_, ModelError>((driver, retry))
        })
        .await?;
    let summary = device.run_until_idle(8)?;
    assert_eq!(summary.last_status, AsyncStepStatus::Idle);
    device.clear_trace();
    Ok(PollFixture {
        owner,
        driver,
        device,
        retry,
    })
}

async fn event_fixture() -> ModelResult<PollFixture> {
    let owner = SoftwareCpu::new();
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    device.enable_tracing();
    let (driver, retry) = owner
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut device, 1, 1).await?;
            let dst = driver.allocate_buffer_on(&mut device, 4).await?;
            driver
                .submit_copy_on(
                    &mut device,
                    CopyRequest::new(BufferId::new(usize::MAX), dst, 4),
                )
                .await?;
            Ok::<_, ModelError>((driver, CopyRequest::new(dst, dst, 4)))
        })
        .await?;
    assert!(matches!(
        device.run_until_idle(8),
        Err(pdma_model_device::ModelError::UnknownBuffer(_))
    ));
    device.clear_trace();
    Ok(PollFixture {
        owner,
        driver,
        device,
        retry,
    })
}

#[tokio::test(flavor = "current_thread")]
async fn off_context_completion_poll_does_not_consume_record_or_credit() -> ModelResult<()> {
    // Given: one published completion owned by a CPU-bound depth-one driver.
    let mut fixture = completion_fixture().await?;

    // When: code outside any software CPU polls before the owner retries.
    let invalid = fixture.driver.poll_completion_on(&mut fixture.device).await;
    let invalid_polls = fixture.trace_count(TraceEventKind::CompletionPolled);
    let retried = fixture.retry_completion().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(ModelError::SoftwareCpuInactive),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_cpu_completion_poll_does_not_consume_record_or_credit() -> ModelResult<()> {
    // Given: one published completion owned by a CPU-bound depth-one driver.
    let mut fixture = completion_fixture().await?;
    let other = SoftwareCpu::new();

    // When: another software CPU polls before the owner retries.
    let invalid = other
        .run_main(fixture.driver.poll_completion_on(&mut fixture.device))
        .await;
    let invalid_polls = fixture.trace_count(TraceEventKind::CompletionPolled);
    let retried = fixture.retry_completion().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(ModelError::SoftwareCpuMismatch),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn off_context_event_poll_does_not_consume_record_or_credit() -> ModelResult<()> {
    // Given: one published event owned by a CPU-bound depth-one driver.
    let mut fixture = event_fixture().await?;

    // When: code outside any software CPU polls before the owner retries.
    let invalid = fixture.driver.poll_event_on(&mut fixture.device).await;
    let invalid_polls = fixture.trace_count(TraceEventKind::EventPolled);
    let retried = fixture.retry_event().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(ModelError::SoftwareCpuInactive),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_cpu_event_poll_does_not_consume_record_or_credit() -> ModelResult<()> {
    // Given: one published event owned by a CPU-bound depth-one driver.
    let mut fixture = event_fixture().await?;
    let other = SoftwareCpu::new();

    // When: another software CPU polls before the owner retries.
    let invalid = other
        .run_main(fixture.driver.poll_event_on(&mut fixture.device))
        .await;
    let invalid_polls = fixture.trace_count(TraceEventKind::EventPolled);
    let retried = fixture.retry_event().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(ModelError::SoftwareCpuMismatch),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}
