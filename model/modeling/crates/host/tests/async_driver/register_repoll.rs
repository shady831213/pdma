use super::register_access::GatedAccess;
use hardware_interface_runtime::{CpuContextError, SharedMemory, SoftwareCpu};
use pdma_model_contract::{
    abi::registers::{QueueRegisters, queue_offset},
    semantic::{CommandId, ResourceConfig},
};
use pdma_model_device::{AsyncDevice, PdmaMemoryProvisioning};
use pdma_model_host::{
    Command, CommandOperation, CopyRequest, DriverPort, PortError, PortResult, QueueConfigHandle,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

#[derive(Clone, Copy)]
enum Operation {
    Create,
    Submit,
    Doorbell,
    Completion,
    Event,
}

#[expect(
    clippy::expect_used,
    reason = "operation-specific fixture preparation must fail when the test cannot reach its poll boundary"
)]
impl Operation {
    fn offset(self) -> u32 {
        let local = match self {
            Self::Create => QueueRegisters::ENABLE.offset(),
            Self::Submit => QueueRegisters::SQ_PRODUCER.offset(),
            Self::Doorbell => QueueRegisters::DOORBELL.offset(),
            Self::Completion => QueueRegisters::CQ_CONSUMER.offset(),
            Self::Event => QueueRegisters::EQ_CONSUMER.offset(),
        };
        queue_offset(0, local).expect("queue register")
    }

    async fn prepare(
        self,
        port: &mut PdmaRegisterPort<GatedAccess>,
    ) -> (Option<QueueConfigHandle>, Command) {
        let queue = match self {
            Self::Create => None,
            Self::Submit | Self::Doorbell | Self::Completion | Self::Event => Some(
                port.create_queue(super::register_flow::config())
                    .await
                    .expect("queue"),
            ),
        };
        let buffer = port.allocate_buffer(1).await.expect("buffer");
        let length = match self {
            Self::Event => 2,
            Self::Create | Self::Submit | Self::Doorbell | Self::Completion => 1,
        };
        let command = Command::new(
            CommandId::new(41),
            CommandOperation::Copy(CopyRequest::new(buffer, buffer, length)),
        );
        match self {
            Self::Doorbell | Self::Completion | Self::Event => {
                let queue = queue.expect("queue");
                port.submit_command(queue, command.clone())
                    .await
                    .expect("submit");
                match self {
                    Self::Completion | Self::Event => {
                        port.ring_doorbell(queue).await.expect("doorbell");
                    }
                    Self::Create | Self::Submit | Self::Doorbell => {}
                }
            }
            Self::Create | Self::Submit => {}
        }
        (queue, command)
    }
}

#[expect(
    clippy::expect_used,
    reason = "the shared poll-boundary test asserts fixture setup and preservation of pending records"
)]
async fn repoll(operation: Operation, wrong_cpu: bool) {
    // Given: a direct hardware access suspends within its owning CPU poll.
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let owner = SoftwareCpu::new();
    let access = GatedAccess::new(AsyncDeviceAccess::new(device.handle()));
    let (mut port, queue, command) = owner
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let (queue, command) = operation.prepare(&mut port).await;
            (port, queue, command)
        })
        .await;
    match operation {
        Operation::Completion => {
            device.wait_for_progress_after(0).await;
        }
        Operation::Event => {
            device.wait_for_error_after(0).await;
        }
        Operation::Create | Operation::Submit | Operation::Doorbell => {}
    }
    access.arm_write(operation.offset());
    let mut future: Pin<Box<dyn Future<Output = PortResult<()>> + Send + '_>> = match operation {
        Operation::Create => Box::pin(async {
            port.create_queue(super::register_flow::config())
                .await
                .map(|_| ())
        }),
        Operation::Submit => Box::pin(port.submit_command(queue.expect("queue"), command)),
        Operation::Doorbell => Box::pin(port.ring_doorbell(queue.expect("queue"))),
        Operation::Completion => Box::pin(async {
            port.poll_completion(queue.expect("queue"))
                .await
                .map(|_| ())
        }),
        Operation::Event => Box::pin(async {
            port.poll_queue_event(queue.expect("queue"))
                .await
                .map(|_| ())
        }),
    };
    owner
        .run_main(poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        }))
        .await;
    assert!(access.is_blocked());
    let accesses = access.accesses();

    // When: the pending operation resumes outside its owning CPU.
    let error = if wrong_cpu {
        SoftwareCpu::new().run_main(future.as_mut()).await
    } else {
        future.as_mut().await
    };
    drop(future);

    // Then: CPU validation stops before another hardware access or record removal.
    let expected = if wrong_cpu {
        CpuContextError::Mismatch
    } else {
        CpuContextError::Inactive
    };
    assert_eq!(error, Err(PortError::Cpu(expected)));
    assert_eq!(access.accesses(), accesses);
    access.release();
    owner
        .run_main(async {
            match operation {
                Operation::Completion => assert_eq!(
                    port.poll_completion(queue.expect("queue"))
                        .await
                        .expect("completion")
                        .expect("preserved record")
                        .command_id(),
                    CommandId::new(41)
                ),
                Operation::Event => assert_eq!(
                    port.poll_queue_event(queue.expect("queue"))
                        .await
                        .expect("event")
                        .expect("preserved record")
                        .command_id(),
                    CommandId::new(41)
                ),
                Operation::Create | Operation::Submit | Operation::Doorbell => {}
            }
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker");
}

#[tokio::test(flavor = "current_thread")]
async fn every_nested_register_poll_rejects_inactive_cpu() {
    for operation in [
        Operation::Create,
        Operation::Submit,
        Operation::Doorbell,
        Operation::Completion,
        Operation::Event,
    ] {
        repoll(operation, false).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn every_nested_register_poll_rejects_wrong_cpu() {
    for operation in [
        Operation::Create,
        Operation::Submit,
        Operation::Doorbell,
        Operation::Completion,
        Operation::Event,
    ] {
        repoll(operation, true).await;
    }
}
