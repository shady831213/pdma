use super::{
    assertions::{assert_line_lifecycle, assert_line_observation, receive_observations},
    support::{DriverBootstrap, HandlerChannels, SharedDelivery, spawn_shared_delivery},
};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{CopyRequest, IrqLine, IrqRoute, IrqVector};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver, DriverError, DriverPort, DriverResult, adapter::abi::AsyncDeviceAccess,
    compat::AsyncDevicePort,
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

#[tokio::test(flavor = "current_thread")]
async fn line_completion_runs_device_main_and_delivery_as_concurrent_actors() -> DriverResult<()> {
    // Given: a line-routed completion and independently running device, main, and delivery actors.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    let access = AsyncDeviceAccess::new(port.clone());
    let compat_port = AsyncDevicePort::new(port.clone());
    port.enable_tracing().await;
    port.clear_trace().await;
    let cpu = Arc::new(SoftwareCpu::new());
    let depth = cpu.local(0usize);
    let vector = IrqVector::new(61);
    let (bootstrap_tx, bootstrap_rx) = oneshot::channel();
    let (ready_tx, ready_rx) = oneshot::channel();
    let (observations_tx, mut observations_rx) = mpsc::channel(2);
    let (milestones_tx, _milestones_rx) = mpsc::channel(4);
    let (done_tx, mut done_rx) = mpsc::channel(2);
    let delivery = spawn_shared_delivery(SharedDelivery {
        access,
        port: compat_port.clone(),
        cpu: cpu.clone(),
        bootstrap: bootstrap_rx,
        ready: ready_tx,
        depth,
        vector,
        handler: HandlerChannels {
            observations: observations_tx,
            milestones: milestones_tx,
            done: done_tx,
            releases: None,
        },
    });

    // When: complete software setup publishes a clone, waits for registration, then submits once.
    let mut main_port = compat_port;
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = AsyncDriver::setup_on(&mut main_port, 1, 4).await?;
            let completion_capability = driver.completion_irq_source();
            let completion_source = completion_capability.source();
            let event_source = driver.event_irq_source().source();
            bootstrap_tx
                .send(DriverBootstrap {
                    driver: driver.clone(),
                    completion_source,
                    event_source,
                })
                .expect("delivery receives its capability clone");
            ready_rx.await.expect("line handler is registered");
            main_port
                .configure_irq_route(completion_capability, IrqRoute::Line(IrqLine::new(vector)))
                .await?;
            main_port.enable_irq_source(completion_capability).await?;
            main_port.unmask_irq_source(completion_capability).await?;
            let src = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut main_port, src, &[4, 3, 2, 1])
                .await?;
            driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            done_rx.recv().await.expect("line handler completes");
            Ok::<_, DriverError>((
                main_port.read_buffer(dst, 0, 4).await?,
                main_port.irq_snapshot().await,
            ))
        })
        .await
    });
    let delivery_handle = delivery
        .handle
        .await
        .expect("delivery publishes its control handle");
    let (copied, irq_snapshot) = main.await.expect("main joins cleanly")?;
    let observations = receive_observations(&mut observations_rx, 1).await;

    // Then: vector dispatch drains the completion, acknowledges the line, and preserves copy data.
    assert_line_observation(&observations, vector);
    assert_eq!(copied, vec![4, 3, 2, 1]);
    assert_eq!(irq_snapshot, None);
    let device_trace = port.trace_events().await;
    let host_trace = delivery_handle.trace_events();
    assert_line_lifecycle(&device_trace, &host_trace);
    delivery_handle.request_shutdown();
    device.request_shutdown().await;
    let summary = delivery.task.await.expect("delivery joins cleanly");
    let device_summary = worker.await.expect("device worker joins cleanly");
    assert_eq!(summary.deliveries(), 1);
    assert_eq!(device_summary.steps, 1);
    assert_eq!(device_summary.errors, 0);
    Ok(())
}
