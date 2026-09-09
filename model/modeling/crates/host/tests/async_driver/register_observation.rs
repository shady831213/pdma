use super::register_access::GatedAccess;
use hardware_interface_runtime::{SharedMemory, SoftwareCpu};
use pdma_model_contract::{
    abi::registers::{QueueRegisters, queue_offset},
    semantic::{CommandId, ResourceConfig},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver, CopyRequest, DriverError, DriverPort, PortError,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterAccessError, PdmaRegisterPort},
};
use std::{error::Error, sync::Arc};

#[expect(
    clippy::panic,
    clippy::expect_used,
    reason = "this assertion helper must fail the test when the committed error cause is absent"
)]
fn assert_committed(error: DriverError) {
    let DriverError::Port(PortError::Backend(backend)) = error else {
        panic!("expected register backend error: {error:?}");
    };
    assert!(matches!(
        backend
            .source()
            .expect("register cause")
            .downcast_ref::<PdmaRegisterAccessError>(),
        Some(PdmaRegisterAccessError::CommittedObservation(_))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn committed_producer_observation_error_retains_accepted_credit() {
    // Given: the producer write commits before its observer reports failure.
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    let worker = tokio::spawn(worker.run());
    let access = GatedAccess::new(AsyncDeviceAccess::new(device.handle(), memory.clone()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1)
                .await
                .expect("driver");
            let buffer = driver
                .allocate_buffer_on(&mut port, 1)
                .await
                .expect("buffer");
            let request = CopyRequest::new(buffer, buffer, 1);
            access.fail_committed(
                queue_offset(0, QueueRegisters::SQ_PRODUCER.offset()).expect("producer"),
            );

            // When: submission returns the typed committed-observation failure.
            assert_committed(
                driver
                    .submit_copy_on(&mut port, request)
                    .await
                    .expect_err("observer failure"),
            );
            access.release();
            let mut clone = driver.clone();

            // Then: its accepted credit remains reserved until the original command retires.
            assert_eq!(
                clone.submit_copy_on(&mut port, request).await,
                Err(DriverError::QueueFull)
            );
            port.ring_doorbell(driver.queue_config_handle())
                .await
                .expect("doorbell");
            device.wait_for_progress_after(0).await;
            assert_eq!(
                clone
                    .poll_completion_on(&mut port)
                    .await
                    .expect("completion")
                    .expect("record")
                    .command_id(),
                CommandId::new(0)
            );
            assert_eq!(
                driver
                    .submit_copy_on(&mut port, request)
                    .await
                    .expect("reclaimed credit"),
                CommandId::new(1)
            );
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker");
}

#[expect(
    clippy::expect_used,
    reason = "the shared completion/event scenario asserts setup success and the exact observer failure"
)]
async fn consumer_failure(fault: bool) {
    // Given: a published retirement record whose consumer write will report observer failure.
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    let worker = tokio::spawn(worker.run());
    let access = GatedAccess::new(AsyncDeviceAccess::new(device.handle(), memory.clone()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1)
                .await
                .expect("driver");
            let buffer = driver
                .allocate_buffer_on(&mut port, 1)
                .await
                .expect("buffer");
            driver
                .submit_copy_on(
                    &mut port,
                    CopyRequest::new(buffer, buffer, if fault { 2 } else { 1 }),
                )
                .await
                .expect("submit");
            if fault {
                device.wait_for_error_after(0).await;
            } else {
                device.wait_for_progress_after(0).await;
            }
            let consumer = if fault {
                QueueRegisters::EQ_CONSUMER.offset()
            } else {
                QueueRegisters::CQ_CONSUMER.offset()
            };
            access.fail_committed(queue_offset(0, consumer).expect("consumer"));

            // When: the hardware consumer commits and the poll returns the observer error.
            let error = if fault {
                driver
                    .poll_event_on(&mut port)
                    .await
                    .expect_err("observer failure")
            } else {
                driver
                    .poll_completion_on(&mut port)
                    .await
                    .expect_err("observer failure")
            };
            assert_committed(error);
            access.release();
            let mut clone = port.clone();

            // Then: host retention returns the consumed record once and refunds its driver credit.
            let record = if fault {
                driver
                    .poll_event_on(&mut clone)
                    .await
                    .expect("event")
                    .map(pdma_model_contract::semantic::EventRecord::command_id)
            } else {
                driver
                    .poll_completion_on(&mut clone)
                    .await
                    .expect("completion")
                    .map(pdma_model_contract::semantic::CompletionRecord::command_id)
            };
            assert_eq!(record, Some(CommandId::new(0)));
            let duplicate = if fault {
                driver
                    .poll_event_on(&mut port)
                    .await
                    .expect("drained")
                    .map(pdma_model_contract::semantic::EventRecord::command_id)
            } else {
                driver
                    .poll_completion_on(&mut port)
                    .await
                    .expect("drained")
                    .map(pdma_model_contract::semantic::CompletionRecord::command_id)
            };
            assert_eq!(duplicate, None);
            assert_eq!(
                driver
                    .submit_copy_on(&mut port, CopyRequest::new(buffer, buffer, 1))
                    .await
                    .expect("reclaimed credit"),
                CommandId::new(1)
            );
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker");
}

#[tokio::test(flavor = "current_thread")]
async fn committed_completion_observation_error_retains_consumed_record() {
    consumer_failure(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn committed_event_observation_error_retains_consumed_record() {
    consumer_failure(true).await;
}
