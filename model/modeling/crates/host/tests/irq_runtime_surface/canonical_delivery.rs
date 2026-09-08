use hardware_interface_runtime::{
    HandlerRegistry, InterruptDispatchError, InterruptDispatcher, RegisterAccess, SharedMemory,
    SoftwareCpu,
};
use pdma_model_contract::abi::registers::GlobalRegisters;
use pdma_model_contract::semantic::{
    DeviceEpoch, IrqNotification, IrqRoute, IrqVector, ResourceConfig, TraceEventKind,
};
use pdma_model_device::{AsyncDevice, PdmaMemoryProvisioning};
use pdma_model_host::adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort};
use pdma_model_host::runtime::interrupt::{
    PdmaInterruptTrace, PdmaIrqHandler, PdmaIrqService, PdmaIrqServiceError,
};
use pdma_model_host::{
    AsyncDriver, CopyRequest, DriverError, DriverIrqRecord, DriverPort, PortError,
};
use std::{
    future::{Future, poll_fn},
    sync::Arc,
    task::Poll,
};

struct FailingHandler;

impl PdmaIrqHandler for FailingHandler {
    type Error = DriverError;

    async fn handle(
        &mut self,
        _notification: IrqNotification,
        records: &[DriverIrqRecord],
    ) -> Result<(), Self::Error> {
        assert!(matches!(records, [DriverIrqRecord::Completion(_)]));
        Err(DriverError::QueueFull)
    }
}

#[expect(
    clippy::expect_used,
    reason = "fixture setup failures must fail the two canonical delivery tests"
)]
async fn submit_message(
    cpu: &SoftwareCpu,
    access: AsyncDeviceAccess,
) -> (PdmaRegisterPort<AsyncDeviceAccess>, AsyncDriver) {
    cpu.run_main(async {
        let mut port = PdmaRegisterPort::new(access).expect("active canonical port");
        let mut driver = AsyncDriver::setup_on(&mut port, 2, 2)
            .await
            .expect("queue setup");
        let source = driver.completion_irq_source();
        port.configure_irq_route(source, IrqRoute::Message(IrqVector::new(9)))
            .await
            .expect("message route");
        port.enable_irq_source(source).await.expect("enable source");
        port.unmask_irq_source(source).await.expect("unmask source");
        let src = driver
            .allocate_buffer_on(&mut port, 4)
            .await
            .expect("source");
        let dst = driver
            .allocate_buffer_on(&mut port, 4)
            .await
            .expect("destination");
        driver
            .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
            .await
            .expect("submit copy");
        (port, driver)
    })
    .await
}

#[tokio::test(flavor = "current_thread")]
async fn canonical_handler_error_traces_failure_without_hardware_ack() {
    // Given: an actual DMA completion and a software handler that rejects its drained record.
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    device.handle().enable_tracing().await;
    let worker = tokio::spawn(worker.run());
    let cpu = Arc::new(SoftwareCpu::new());
    let access = AsyncDeviceAccess::new(device.handle());
    let (port, driver) = submit_message(&cpu, access.clone()).await;
    let registry = HandlerRegistry::with_handler(
        IrqVector::new(9),
        PdmaIrqService::new(driver, port, FailingHandler),
    )
    .freeze()
    .expect("nonempty registry");
    let trace = Arc::new(PdmaInterruptTrace::default());
    let (_shutdown, receiver) = InterruptDispatcher::shutdown_channel();

    // When: the canonical interrupt output dispatches the completion to the failing handler.
    let result = InterruptDispatcher::run(cpu, registry, access, trace.clone(), receiver).await;

    // Then: failure terminates delivery, emits an error lifecycle, and leaves ACK unissued.
    assert!(matches!(
        result,
        Err(InterruptDispatchError::Handler {
            source: PdmaIrqServiceError::Handler(DriverError::QueueFull),
            ..
        })
    ));
    let events = trace.events();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.kind(), event.detail()))
            .collect::<Vec<_>>(),
        [
            (TraceEventKind::IrqHandlerStarted, "message"),
            (TraceEventKind::IrqHandlerCompleted, "Error"),
        ]
    );
    assert!(
        !device
            .handle()
            .trace_events()
            .await
            .iter()
            .any(|event| event.kind() == TraceEventKind::IrqDeviceAck)
    );
    device.request_shutdown().await;
    worker.await.expect("worker joined");
}

#[tokio::test(flavor = "current_thread")]
async fn canonical_claimed_notification_after_reset_fails_before_drain_and_ack() {
    // Given: an actual claimed message waiting behind another complete handler turn.
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    device.handle().enable_tracing().await;
    let worker = tokio::spawn(worker.run());
    let cpu = Arc::new(SoftwareCpu::new());
    let access = AsyncDeviceAccess::new(device.handle());
    let (port, driver) = submit_message(&cpu, access.clone()).await;
    assert!(device.handle().wait_irq_notification_ready().await);
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let holder_cpu = Arc::clone(&cpu);
    let holder = tokio::spawn(async move {
        holder_cpu
            .run_handler_turn(async {
                entered.send(()).expect("holder entered");
                release_rx.await.expect("holder released");
            })
            .await;
    });
    entered_rx.await.expect("handler holds CPU");
    let registry = HandlerRegistry::with_handler(
        IrqVector::new(9),
        PdmaIrqService::new(driver, port, FailingHandler),
    )
    .freeze()
    .expect("nonempty registry");
    let trace = Arc::new(PdmaInterruptTrace::default());
    let (_shutdown, receiver) = InterruptDispatcher::shutdown_channel();
    let mut delivery = Box::pin(InterruptDispatcher::run(
        cpu,
        registry,
        access.clone(),
        trace.clone(),
        receiver,
    ));
    poll_fn(|context| {
        assert!(delivery.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(!device.handle().irq_notification_ready().await);

    // When: hardware resets after claim but before the accepted handler obtains the CPU.
    RegisterAccess::write(
        &access,
        u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
        &1u32.to_le_bytes(),
    )
    .await
    .expect("hardware reset");
    device.handle().clear_trace().await;
    release.send(()).expect("release holder");
    let result = delivery.await;

    // Then: the old port reports its stale epoch without reading a record or acknowledging.
    assert!(matches!(result, Err(InterruptDispatchError::Handler {
        source: PdmaIrqServiceError::Drain(DriverError::Port(PortError::StaleEpoch { expected, actual })), ..
    }) if expected == DeviceEpoch::new(1) && actual == DeviceEpoch::initial()));
    assert!(
        !device
            .handle()
            .trace_events()
            .await
            .iter()
            .any(|event| matches!(
                event.kind(),
                TraceEventKind::CompletionPolled | TraceEventKind::IrqDeviceAck
            ))
    );
    assert_eq!(
        trace
            .events()
            .last()
            .map(pdma_model_contract::semantic::TraceEvent::detail),
        Some("Error")
    );
    holder.await.expect("holder joined");
    device.request_shutdown().await;
    worker.await.expect("worker joined");
}
