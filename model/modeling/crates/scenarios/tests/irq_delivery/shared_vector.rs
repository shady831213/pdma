use super::{
    assertions::{
        assert_redundant_message_lifecycle, assert_redundant_observations,
        assert_shared_source_observations, receive_observations,
    },
    support::{DriverBootstrap, HandlerChannels, SharedDelivery, spawn_shared_delivery},
};
use pdma_model_contract::{CopyRequest, IrqRoute, IrqVector};
use pdma_model_device::AsyncDevice;
use pdma_model_host::ModelResult;
use pdma_model_host::{AsyncDriver, SoftwareCpu};
use tokio::sync::{mpsc, oneshot};

#[tokio::test(flavor = "current_thread")]
async fn completion_and_event_sources_share_one_persistent_vector_handler() -> ModelResult<()> {
    // Given: both driver sources route to one vector and one persistent handler instance.
    let (device, worker) = fresh_device();
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    let cpu = SoftwareCpu::new();
    let depth = cpu.local(0usize);
    let vector = IrqVector::new(51);
    let (bootstrap_tx, bootstrap_rx) = oneshot::channel();
    let (ready_tx, ready_rx) = oneshot::channel();
    let (observations_tx, mut observations_rx) = mpsc::channel(4);
    let (milestones_tx, _milestones_rx) = mpsc::channel(8);
    let (done_tx, mut done_rx) = mpsc::channel(4);
    let delivery = spawn_shared_delivery(SharedDelivery {
        port: port.clone(),
        cpu: cpu.clone(),
        bootstrap: bootstrap_rx,
        ready: ready_tx,
        depth: depth.clone(),
        vector,
        handler: HandlerChannels {
            observations: observations_tx,
            milestones: milestones_tx,
            done: done_tx,
            releases: None,
        },
    });

    // When: retained main submits one success and one fault through depth-one accounting.
    let mut main_port = port.clone();
    let main_depth = depth.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = AsyncDriver::setup_on(&mut main_port, 1, 4).await?;
            let completion_capability = driver.completion_irq_source();
            let event_capability = driver.event_irq_source();
            let completion_source = completion_capability.source();
            let event_source = event_capability.source();
            bootstrap_tx
                .send(DriverBootstrap {
                    driver: driver.clone(),
                    completion_source,
                    event_source,
                })
                .expect("delivery receives a shared accounting capability");
            ready_rx.await.expect("shared vector is registered");
            for source in [completion_capability, event_capability] {
                main_port
                    .configure_irq_route(source, IrqRoute::Message(vector))
                    .await?;
                main_port.enable_irq_source(source).await?;
                main_port.unmask_irq_source(source).await?;
            }
            let src = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut main_port, src, &[1, 3, 5, 7])
                .await?;
            driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 4))
                .await?;
            done_rx.recv().await.expect("completion handler turn ends");
            driver
                .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 8))
                .await?;
            done_rx.recv().await.expect("event handler turn ends");
            Ok::<_, pdma_model_host::ModelError>((
                main_depth.with(|depth| *depth)?,
                main_port.irq_snapshot().await,
                main_port.read_buffer(dst, 0, 4).await?,
            ))
        })
        .await
    });
    let delivery_handle = delivery
        .handle
        .await
        .expect("delivery publishes its control handle");
    let (final_depth, irq_snapshot, copied) = main.await.expect("main joins cleanly")?;
    let observations = receive_observations(&mut observations_rx, 2).await;

    // Then: call state persists while source demultiplexing drains and acknowledges both rings.
    assert_shared_source_observations(&observations, vector);
    assert_eq!(final_depth, 0);
    assert_eq!(irq_snapshot, None);
    assert_eq!(copied, vec![1, 3, 5, 7]);
    delivery_handle.request_shutdown();
    device.request_shutdown().await;
    let summary = delivery
        .task
        .await
        .expect("delivery joins cleanly")
        .expect("bootstrap sender stays alive")?;
    let device_summary = worker.await.expect("device worker joins cleanly");
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(device_summary.steps, 1);
    assert_eq!(device_summary.errors, 1);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn shared_vector_redundant_messages_use_one_persistent_handler() -> ModelResult<()> {
    // Given: two masked fault messages accumulate for one source and one registered vector.
    let (device, worker) = fresh_device();
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    port.enable_tracing().await;
    port.clear_trace().await;
    let cpu = SoftwareCpu::new();
    let depth = cpu.local(0usize);
    let vector = IrqVector::new(52);
    let (bootstrap_tx, bootstrap_rx) = oneshot::channel();
    let (ready_tx, ready_rx) = oneshot::channel();
    let (submitted_tx, submitted_rx) = oneshot::channel();
    let (unmask_tx, unmask_rx) = oneshot::channel();
    let (observations_tx, mut observations_rx) = mpsc::channel(4);
    let (milestones_tx, _milestones_rx) = mpsc::channel(8);
    let (done_tx, mut done_rx) = mpsc::channel(4);
    let delivery = spawn_shared_delivery(SharedDelivery {
        port: port.clone(),
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

    // When: main publishes both faults while masked, then exposes both pending messages.
    let mut main_port = port.clone();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let mut driver = AsyncDriver::setup_on(&mut main_port, 2, 4).await?;
            let completion_source = driver.completion_irq_source().source();
            let event_capability = driver.event_irq_source();
            let event_source = event_capability.source();
            bootstrap_tx
                .send(DriverBootstrap {
                    driver: driver.clone(),
                    completion_source,
                    event_source,
                })
                .expect("delivery receives its capability clone");
            ready_rx.await.expect("shared handler is registered");
            main_port
                .configure_irq_route(event_capability, IrqRoute::Message(vector))
                .await?;
            main_port.enable_irq_source(event_capability).await?;
            let src = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut main_port, 4)
                .await
                .expect("allocation");
            for _ in 0..2 {
                driver
                    .submit_copy_on(&mut main_port, CopyRequest::new(src, dst, 8))
                    .await?;
            }
            submitted_tx
                .send(())
                .expect("harness observes both submissions");
            unmask_rx.await.expect("harness releases pending messages");
            main_port.unmask_irq_source(event_capability).await?;
            done_rx.recv().await.expect("first message is serviced");
            done_rx.recv().await.expect("redundant message is serviced");
            Ok::<_, pdma_model_host::ModelError>(main_port.irq_snapshot().await)
        })
        .await
    });
    let delivery_handle = delivery
        .handle
        .await
        .expect("delivery publishes its control handle");
    submitted_rx.await.expect("main publishes both commands");
    let error_snapshot = device.wait_for_error_after(1).await;
    assert_eq!(error_snapshot.errors, 2);
    unmask_tx
        .send(())
        .expect("main may unmask the event source");
    assert_eq!(main.await.expect("main joins cleanly")?, None);
    let observations = receive_observations(&mut observations_rx, 2).await;

    // Then: the persistent handler drains both events once and safely accepts the redundant turn.
    assert_redundant_observations(&observations, vector);
    let device_trace = port.trace_events().await;
    assert_redundant_message_lifecycle(&device_trace);
    delivery_handle.request_shutdown();
    device.request_shutdown().await;
    let summary = delivery
        .task
        .await
        .expect("delivery joins cleanly")
        .expect("bootstrap sender stays alive")?;
    let device_summary = worker.await.expect("device worker joins cleanly");
    assert_eq!(summary.deliveries(), 2);
    assert_eq!(device_summary.errors, 2);
    Ok(())
}

fn fresh_device() -> (AsyncDevice, pdma_model_device::AsyncDeviceWorker) {
    AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ))
}
