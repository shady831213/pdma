use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{BufferId, CommandId, CopyRequest, TraceEventKind};
use pdma_model_device::{AsyncDevice, AsyncDeviceWorker};
use pdma_model_host::{AsyncDriver, DriverError, DriverResult, compat::AsyncDevicePort};

#[derive(Clone, Copy)]
enum InvalidContext {
    Inactive,
    WrongCpu,
}

impl InvalidContext {
    const fn error(self) -> DriverError {
        match self {
            Self::Inactive => {
                DriverError::InactiveCpu(hardware_interface_runtime::CpuContextError::Inactive)
            }
            Self::WrongCpu => {
                DriverError::WrongCpu(hardware_interface_runtime::CpuContextError::Mismatch)
            }
        }
    }
}

struct PollFixture {
    owner: SoftwareCpu,
    driver: AsyncDriver,
    device: AsyncDevice,
    worker: tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary>,
    port: AsyncDevicePort,
    retry: CopyRequest,
}

impl PollFixture {
    async fn trace_count(&self, kind: TraceEventKind) -> usize {
        self.device
            .handle()
            .trace_events()
            .await
            .iter()
            .filter(|event| event.kind() == kind)
            .count()
    }

    async fn retry_completion(&mut self) -> DriverResult<(Option<CommandId>, CommandId)> {
        self.owner
            .run_main(async {
                let completion = self
                    .driver
                    .poll_completion_on(&mut self.port)
                    .await?
                    .map(pdma_model_contract::semantic::CompletionRecord::command_id);
                let submitted = self
                    .driver
                    .submit_copy_on(&mut self.port, self.retry)
                    .await?;
                Ok((completion, submitted))
            })
            .await
    }

    async fn retry_event(&mut self) -> DriverResult<(Option<CommandId>, CommandId)> {
        self.owner
            .run_main(async {
                let event = self
                    .driver
                    .poll_event_on(&mut self.port)
                    .await?
                    .map(pdma_model_contract::semantic::EventRecord::command_id);
                let submitted = self
                    .driver
                    .submit_copy_on(&mut self.port, self.retry)
                    .await?;
                Ok((event, submitted))
            })
            .await
    }

    async fn shutdown(self) {
        self.device.request_shutdown().await;
        assert!(self.worker.await.is_ok());
    }
}

fn new_device() -> (AsyncDevice, AsyncDeviceWorker) {
    AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    )
}

async fn completion_fixture() -> DriverResult<PollFixture> {
    let owner = SoftwareCpu::new();
    let (device, worker) = new_device();
    let handle = device.handle();
    handle.enable_tracing().await;
    let mut port = AsyncDevicePort::new(handle.clone());
    let (driver, retry) = owner
        .run_main(async {
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let src = driver.allocate_buffer_on(&mut port, 4).await?;
            let dst = driver.allocate_buffer_on(&mut port, 4).await?;
            let retry = CopyRequest::new(src, dst, 4);
            driver.submit_copy_on(&mut port, retry).await?;
            Ok::<_, DriverError>((driver, retry))
        })
        .await?;
    let worker = tokio::spawn(worker.run());
    device.wait_for_progress_after(0).await;
    handle.clear_trace().await;
    Ok(PollFixture {
        owner,
        driver,
        device,
        worker,
        port,
        retry,
    })
}

async fn event_fixture() -> DriverResult<PollFixture> {
    let owner = SoftwareCpu::new();
    let (device, worker) = new_device();
    let handle = device.handle();
    handle.enable_tracing().await;
    let mut port = AsyncDevicePort::new(handle.clone());
    let (driver, retry) = owner
        .run_main(async {
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
    let worker = tokio::spawn(worker.run());
    device.wait_for_error_after(0).await;
    handle.clear_trace().await;
    Ok(PollFixture {
        owner,
        driver,
        device,
        worker,
        port,
        retry,
    })
}

async fn completion_preflight(context: InvalidContext) -> DriverResult<()> {
    // Given: one published completion owned by a CPU-bound depth-one driver.
    let mut fixture = completion_fixture().await?;

    // When: the first poll occurs outside the owning software CPU.
    let invalid = match context {
        InvalidContext::Inactive => fixture.driver.poll_completion_on(&mut fixture.port).await,
        InvalidContext::WrongCpu => {
            SoftwareCpu::new()
                .run_main(fixture.driver.poll_completion_on(&mut fixture.port))
                .await
        }
    };
    let invalid_polls = fixture.trace_count(TraceEventKind::CompletionPolled).await;
    let retried = fixture.retry_completion().await;

    // Then: validation precedes the destructive pop and preserves record plus credit.
    assert_eq!(invalid, Err(context.error()));
    assert_eq!(invalid_polls, 0);
    assert_eq!(retried, Ok((Some(CommandId::new(0)), CommandId::new(1))));
    fixture.shutdown().await;
    Ok(())
}

async fn event_preflight(context: InvalidContext) -> DriverResult<()> {
    // Given: one published event owned by a CPU-bound depth-one driver.
    let mut fixture = event_fixture().await?;

    // When: the first poll occurs outside the owning software CPU.
    let invalid = match context {
        InvalidContext::Inactive => fixture.driver.poll_event_on(&mut fixture.port).await,
        InvalidContext::WrongCpu => {
            SoftwareCpu::new()
                .run_main(fixture.driver.poll_event_on(&mut fixture.port))
                .await
        }
    };
    let invalid_polls = fixture.trace_count(TraceEventKind::EventPolled).await;
    let retried = fixture.retry_event().await;

    // Then: validation precedes the destructive pop and preserves record plus credit.
    assert_eq!(invalid, Err(context.error()));
    assert_eq!(invalid_polls, 0);
    assert_eq!(retried, Ok((Some(CommandId::new(0)), CommandId::new(1))));
    fixture.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn off_context_completion_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    completion_preflight(InvalidContext::Inactive).await
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_cpu_completion_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    completion_preflight(InvalidContext::WrongCpu).await
}

#[tokio::test(flavor = "current_thread")]
async fn off_context_event_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    event_preflight(InvalidContext::Inactive).await
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_cpu_event_poll_does_not_consume_record_or_credit() -> DriverResult<()> {
    event_preflight(InvalidContext::WrongCpu).await
}
