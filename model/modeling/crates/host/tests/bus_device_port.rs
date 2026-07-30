//! Behavioral contract for the ABI-backed asynchronous device port.

#[path = "bus_device_port/cancellation_support.rs"]
/// Test-only ABI cancellation observations retained outside the integration-test root.
pub mod cancellation_support;
#[path = "bus_device_port/consumer_race_support.rs"]
/// Deterministic clone-consumer race scenarios.
pub mod consumer_race_support;
#[path = "bus_device_port/interleaving_support.rs"]
/// Test-only CPU interleaving scenarios.
pub mod interleaving_support;

use pdma_model_contract::abi::{DeviceEpoch, WholeDeviceReset};
use pdma_model_contract::{
    BroadcastCopyRequest, BroadcastDestinationRange, CopyRequest, IrqRoute, IrqVector, ModelError,
    ModelResult, StridedCopyRequest, VectorAddRequest, VectorBufferRange,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{AsyncDevicePort, AsyncDriver, BusDevicePort, SoftwareCpu};

struct HappyFixture {
    port: BusDevicePort<pdma_model_device::AsyncDeviceHandle>,
    driver: AsyncDriver,
    destinations: [pdma_model_contract::BufferId; 5],
    commands: [pdma_model_contract::CommandId; 4],
}

async fn submit_all_operations(
    handle: pdma_model_device::AsyncDeviceHandle,
    cpu: &SoftwareCpu,
) -> ModelResult<HappyFixture> {
    cpu.run_main(async {
        let mut port = BusDevicePort::new(handle)?;
        let mut driver = AsyncDriver::setup_on(&mut port, 4, 4).await?;
        let completion = driver.completion_irq_source();
        port.configure_irq_route(completion, IrqRoute::Message(IrqVector::new(23)))
            .await?;
        port.enable_irq_source(completion).await?;
        port.unmask_irq_source(completion).await?;
        let source = driver.allocate_buffer_on(&mut port, 4).await?;
        let source_b = driver.allocate_buffer_on(&mut port, 4).await?;
        let copy_dst = driver.allocate_buffer_on(&mut port, 4).await?;
        let strided_dst = driver.allocate_buffer_on(&mut port, 4).await?;
        let broadcast_a = driver.allocate_buffer_on(&mut port, 4).await?;
        let broadcast_b = driver.allocate_buffer_on(&mut port, 4).await?;
        let vector_dst = driver.allocate_buffer_on(&mut port, 4).await?;
        driver
            .write_buffer_on(&mut port, source, &[1, 3, 5, 7])
            .await?;
        driver
            .write_buffer_on(&mut port, source_b, &[2, 4, 6, 8])
            .await?;
        let copy = driver
            .submit_copy_on(&mut port, CopyRequest::new(source, copy_dst, 4))
            .await?;
        let strided = driver
            .submit_strided_copy_on(
                &mut port,
                StridedCopyRequest::new(source, strided_dst, 2, 2, 2, 2),
            )
            .await?;
        let broadcast = driver
            .submit_broadcast_copy_on(
                &mut port,
                BroadcastCopyRequest::new(
                    source,
                    0,
                    4,
                    vec![
                        BroadcastDestinationRange::new(broadcast_a, 0),
                        BroadcastDestinationRange::new(broadcast_b, 0),
                    ],
                ),
            )
            .await?;
        let vector = driver
            .submit_vector_add_on(
                &mut port,
                VectorAddRequest::new(
                    VectorBufferRange::new(source, 0),
                    VectorBufferRange::new(source_b, 0),
                    VectorBufferRange::new(vector_dst, 0),
                    4,
                ),
            )
            .await?;
        Ok(HappyFixture {
            port,
            driver,
            destinations: [copy_dst, strided_dst, broadcast_a, broadcast_b, vector_dst],
            commands: [copy, strided, broadcast, vector],
        })
    })
    .await
}

#[tokio::test(flavor = "current_thread")]
async fn region_handoff_drives_all_operations_and_irq_service_through_one_clone_shared_state()
-> ModelResult<()> {
    // Given: one real ABI bus and a CPU-bound driver/port pair.
    let (device, worker) = AsyncDevice::queue_port();
    let cpu = SoftwareCpu::new();
    let worker_task = tokio::spawn(async move { worker.run().await });

    // When: main submits through mapped IOVAs, then a handler clone drains the Message IRQ.
    let HappyFixture {
        port: main_port,
        mut driver,
        destinations,
        commands,
    } = submit_all_operations(device.handle(), &cpu).await?;
    let mut handler_port = main_port.clone();
    let mut records = Vec::new();
    while records.len() < 4 {
        let notification = handler_port
            .wait_irq_notification()
            .await
            .ok_or(ModelError::IrqDeliveryUnavailable)?;
        records.extend(
            cpu.run_main(driver.service_irq_on(&mut handler_port, notification))
                .await?,
        );
    }
    let mut payloads = Vec::new();
    for destination in destinations {
        payloads.push(
            cpu.run_main(driver.read_buffer_on(&main_port, destination, 4))
                .await?,
        );
    }

    // Then: descriptor execution, canonical decode, consumer commit, and clone state agree.
    assert_eq!(
        commands.map(pdma_model_contract::CommandId::raw),
        [0, 1, 2, 3]
    );
    assert_eq!(records.len(), 4);
    assert_eq!(
        payloads,
        vec![
            vec![1, 3, 5, 7],
            vec![1, 3, 5, 7],
            vec![1, 3, 5, 7],
            vec![1, 3, 5, 7],
            vec![3, 7, 11, 15],
        ]
    );
    device.request_shutdown().await;
    worker_task
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_before_and_after_atomic_acceptance_preserves_exact_commit_points()
-> ModelResult<()> {
    cancellation_support::verify_cancellation_commit_points().await
}

#[tokio::test(flavor = "current_thread")]
async fn clone_shared_completion_and_event_consumer_races_reject_stale_main() -> ModelResult<()> {
    for lane in [
        consumer_race_support::ConsumerLane::Completion,
        consumer_race_support::ConsumerLane::Event,
    ] {
        consumer_race_support::run_consumer_race(lane).await?;
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_cpu_and_handler_repoll_commit_nothing() -> ModelResult<()> {
    // Given: one owner-bound port before its first raw allocation.
    let (device, _worker) = AsyncDevice::queue_port();
    let owner = SoftwareCpu::new();
    let mut port = owner
        .run_main(async { BusDevicePort::new(device.handle()) })
        .await?;
    let mut clone = port.clone();
    let other = SoftwareCpu::new();

    // When: inactive and wrong-CPU polls precede an owner repoll through the clone.
    let inactive = port.allocate_buffer(4).await;
    let wrong = other.run_main(clone.allocate_buffer(4)).await;
    let accepted = owner.run_main(clone.allocate_buffer(4)).await?;

    // Then: both invalid polls fail before raw allocation and the owner sees object zero.
    assert_eq!(inactive, Err(ModelError::SoftwareCpuInactive));
    assert_eq!(wrong, Err(ModelError::SoftwareCpuMismatch));
    assert_eq!(accepted.raw(), 0);
    interleaving_support::verify_handler_turn_rejects_pending_port_repoll().await?;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn reset_stales_all_clones_without_mutex_deadlock() -> ModelResult<()> {
    // Given: two old port clones, a live epoch-zero queue, and persistent mapped bytes.
    let (device, _worker) = AsyncDevice::queue_port();
    let handle = device.handle();
    let cpu = SoftwareCpu::new();
    let (mut old, mut clone, driver, buffer) = cpu
        .run_main(async {
            let mut port = BusDevicePort::new(handle.clone())?;
            let driver = AsyncDriver::setup_on(&mut port, 1, 1).await?;
            let buffer = driver.allocate_buffer_on(&mut port, 4).await?;
            driver
                .write_buffer_on(&mut port, buffer, &[9, 8, 7, 6])
                .await?;
            Ok::<_, ModelError>((port.clone(), port, driver, buffer))
        })
        .await?;
    let next = handle
        .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;
    let current = handle
        .reset(WholeDeviceReset::new(next))
        .await
        .map_err(|_| ModelError::DmaSubmissionState)?;

    // When: both epoch-zero clones cross two resets and a fresh epoch-two port bootstraps.
    let stale_old = cpu
        .run_main(old.ring_doorbell(driver.queue_config_handle()))
        .await;
    let stale_clone = cpu
        .run_main(clone.ring_doorbell(driver.queue_config_handle()))
        .await;
    let bytes = cpu.run_main(driver.read_buffer_on(&old, buffer, 4)).await?;
    let fresh_queue = cpu
        .run_main(async {
            let mut fresh = BusDevicePort::new_at(handle, current)?;
            AsyncDriver::setup_on(&mut fresh, 1, 1).await
        })
        .await?;

    // Then: transport reports its actual current epoch and the supplied stale capability.
    let stale = Err(ModelError::StaleDeviceEpoch {
        expected: current,
        actual: DeviceEpoch::initial(),
    });
    assert_eq!(stale_old, stale);
    assert_eq!(stale_clone, stale);
    assert_eq!(bytes, vec![9, 8, 7, 6]);
    assert_eq!(fresh_queue.queue_id().raw(), 0);
    Ok(())
}
