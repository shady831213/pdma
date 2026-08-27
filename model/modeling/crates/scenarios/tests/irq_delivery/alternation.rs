use super::{
    alternation_program::{MainProgram, assert_milestone_order, spawn_main},
    assertions::{assert_distinct_observations, receive_observations, trace_count},
    support::{DistinctDelivery, HandlerChannels, spawn_distinct_delivery},
};
use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{IrqVector, TraceEventKind};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{DriverResult, adapters::AsyncDeviceAccess, compat::AsyncDevicePort};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

#[tokio::test(flavor = "current_thread")]
async fn distinct_vectors_repeat_main_handler_alternation_with_shared_depth_one_accounting()
-> DriverResult<()> {
    // Given: one retained main capability and handlers registered for distinct MSI-style vectors.
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let port = device.handle();
    let access = AsyncDeviceAccess::new(port.clone());
    let compat_port = AsyncDevicePort::new(port.clone());
    port.enable_tracing().await;
    port.clear_trace().await;
    let cpu = Arc::new(SoftwareCpu::new());
    let depth = cpu.local(0usize);
    let vector_a = IrqVector::new(41);
    let vector_b = IrqVector::new(42);
    let (bootstrap_tx, bootstrap_rx) = oneshot::channel();
    let (ready_tx, ready_rx) = oneshot::channel();
    let (observations_tx, mut observations_rx) = mpsc::channel(8);
    let (milestones_tx, mut milestones_rx) = mpsc::channel(32);
    let (a_done_tx, a_done_rx) = mpsc::channel(4);
    let (b_done_tx, b_done_rx) = mpsc::channel(4);
    let (a_release_tx, a_release_rx) = mpsc::channel(4);
    let delivery = spawn_distinct_delivery(DistinctDelivery {
        access,
        port: compat_port.clone(),
        cpu: cpu.clone(),
        bootstrap: bootstrap_rx,
        ready: ready_tx,
        depth: depth.clone(),
        vector_a,
        vector_b,
        handler_a: HandlerChannels {
            observations: observations_tx.clone(),
            milestones: milestones_tx.clone(),
            done: a_done_tx,
            releases: Some(a_release_rx),
        },
        handler_b: HandlerChannels {
            observations: observations_tx,
            milestones: milestones_tx.clone(),
            done: b_done_tx,
            releases: None,
        },
    });

    // When: main submits completion and event work in two consecutive A/B rounds.
    let main = spawn_main(MainProgram {
        port: compat_port,
        cpu: cpu.clone(),
        depth: depth.clone(),
        bootstrap: bootstrap_tx,
        ready: ready_rx,
        milestones: milestones_tx,
        a_done: a_done_rx,
        b_done: b_done_rx,
        vector_a,
        vector_b,
    });
    let delivery_handle = delivery
        .handle
        .await
        .expect("delivery publishes its host control handle");
    assert_milestone_order(&mut milestones_rx, &a_release_tx).await;
    let outcome = main.await.expect("software main joins cleanly")?;
    let observations = receive_observations(&mut observations_rx, 5).await;

    // Then: every CPU turn is ordered, non-nested, acknowledged, and shares one depth-one budget.
    assert_distinct_observations(&observations, vector_a, vector_b);
    assert_eq!(outcome.copied, vec![9, 8, 7, 6]);
    let device_trace = port.trace_events().await;
    assert_eq!(
        trace_count(&device_trace, TraceEventKind::IrqMessageEmitted),
        5
    );
    assert_eq!(trace_count(&device_trace, TraceEventKind::IrqDeviceAck), 5);
    let host_trace = delivery_handle.trace_events();
    assert_eq!(
        trace_count(&host_trace, TraceEventKind::IrqHandlerStarted),
        5
    );
    assert_eq!(
        trace_count(&host_trace, TraceEventKind::IrqHandlerCompleted),
        5
    );
    delivery_handle.request_shutdown();
    device.request_shutdown().await;
    let summary = delivery.task.await.expect("delivery joins cleanly");
    let device_summary = worker.await.expect("device worker joins cleanly");
    assert_eq!(summary.deliveries(), 5);
    assert_eq!(device_summary.steps, 3);
    assert_eq!(device_summary.errors, 2);
    assert!(outcome.retained_submit);
    assert!(outcome.irq_cleared);
    assert_eq!(outcome.final_depth, 0);
    Ok(())
}
