use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{BufferId, CommandId, CopyRequest, TraceEventKind};
use pdma_model_device::{AsyncStepStatus, DeviceError, SyncDevice};
use pdma_model_host::DriverError;
use pdma_model_host::DriverResult;
use pdma_model_host::{AsyncDriver, PortError, compat::SemanticDevicePort};

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
    ) -> DriverResult<(Option<CommandId>, DriverResult<CommandId>)> {
        self.owner
            .run_main(async {
                let mut port = SemanticDevicePort::new(&mut self.device);
                let completion = self
                    .driver
                    .poll_completion_on(&mut port)
                    .await?
                    .map(pdma_model_contract::semantic::CompletionRecord::command_id);
                let submitted = self.driver.submit_copy_on(&mut port, self.retry).await;
                Ok((completion, submitted))
            })
            .await
    }

    async fn retry_event(&mut self) -> DriverResult<(Option<CommandId>, DriverResult<CommandId>)> {
        self.owner
            .run_main(async {
                let mut port = SemanticDevicePort::new(&mut self.device);
                let event = self
                    .driver
                    .poll_event_on(&mut port)
                    .await?
                    .map(pdma_model_contract::semantic::EventRecord::command_id);
                let submitted = self.driver.submit_copy_on(&mut port, self.retry).await;
                Ok((event, submitted))
            })
            .await
    }
}

async fn completion_fixture() -> DriverResult<PollFixture> {
    let owner = SoftwareCpu::new();
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    device.enable_tracing();
    let (driver, retry) = owner
        .run_main(async {
            let mut port = SemanticDevicePort::new(&mut device);
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let src = driver.allocate_buffer_on(&mut port, 4).await?;
            let dst = driver.allocate_buffer_on(&mut port, 4).await?;
            let retry = CopyRequest::new(src, dst, 4);
            driver.submit_copy_on(&mut port, retry).await?;
            Ok::<_, DriverError>((driver, retry))
        })
        .await?;
    let summary = device
        .run_until_idle(8)
        .map_err(|_| DriverError::Port(PortError::SubmissionState))?;
    assert_eq!(summary.last_status, AsyncStepStatus::Idle);
    device.clear_trace();
    Ok(PollFixture {
        owner,
        driver,
        device,
        retry,
    })
}

async fn event_fixture() -> DriverResult<PollFixture> {
    let owner = SoftwareCpu::new();
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    device.enable_tracing();
    let (driver, retry) = owner
        .run_main(async {
            let mut port = SemanticDevicePort::new(&mut device);
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let dst = driver.allocate_buffer_on(&mut port, 4).await?;
            driver
                .submit_copy_on(
                    &mut port,
                    CopyRequest::new(BufferId::new(usize::MAX), dst, 4),
                )
                .await?;
            Ok::<_, DriverError>((driver, CopyRequest::new(dst, dst, 4)))
        })
        .await?;
    assert!(matches!(
        device.run_until_idle(8),
        Err(DeviceError::UnknownBuffer(_))
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
async fn off_context_completion_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    // Given: one published completion owned by a CPU-bound depth-one driver.
    let mut fixture = completion_fixture().await?;

    // When: code outside any software CPU polls before the owner retries.
    let invalid = {
        let mut port = SemanticDevicePort::new(&mut fixture.device);
        fixture.driver.poll_completion_on(&mut port).await
    };
    let invalid_polls = fixture.trace_count(TraceEventKind::CompletionPolled);
    let retried = fixture.retry_completion().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(DriverError::InactiveCpu(
                hardware_interface_runtime::CpuContextError::Inactive
            )),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_cpu_completion_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    // Given: one published completion owned by a CPU-bound depth-one driver.
    let mut fixture = completion_fixture().await?;
    let other = SoftwareCpu::new();

    // When: another software CPU polls before the owner retries.
    let invalid = {
        let mut port = SemanticDevicePort::new(&mut fixture.device);
        other
            .run_main(fixture.driver.poll_completion_on(&mut port))
            .await
    };
    let invalid_polls = fixture.trace_count(TraceEventKind::CompletionPolled);
    let retried = fixture.retry_completion().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(DriverError::WrongCpu(
                hardware_interface_runtime::CpuContextError::Mismatch
            )),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn off_context_event_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    // Given: one published event owned by a CPU-bound depth-one driver.
    let mut fixture = event_fixture().await?;

    // When: code outside any software CPU polls before the owner retries.
    let invalid = {
        let mut port = SemanticDevicePort::new(&mut fixture.device);
        fixture.driver.poll_event_on(&mut port).await
    };
    let invalid_polls = fixture.trace_count(TraceEventKind::EventPolled);
    let retried = fixture.retry_event().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(DriverError::InactiveCpu(
                hardware_interface_runtime::CpuContextError::Inactive
            )),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_cpu_event_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    // Given: one published event owned by a CPU-bound depth-one driver.
    let mut fixture = event_fixture().await?;
    let other = SoftwareCpu::new();

    // When: another software CPU polls before the owner retries.
    let invalid = {
        let mut port = SemanticDevicePort::new(&mut fixture.device);
        other
            .run_main(fixture.driver.poll_event_on(&mut port))
            .await
    };
    let invalid_polls = fixture.trace_count(TraceEventKind::EventPolled);
    let retried = fixture.retry_event().await;

    // Then: validation fails before the port pop, preserving both record and credit.
    assert_eq!(
        (invalid, invalid_polls, retried),
        (
            Err(DriverError::WrongCpu(
                hardware_interface_runtime::CpuContextError::Mismatch
            )),
            0,
            Ok((Some(CommandId::new(0)), Ok(CommandId::new(1)))),
        )
    );
    Ok(())
}
