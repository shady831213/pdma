use super::{AccessError, Managed, control::rings};
use crate::harness::TestResult;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAllocator, SoftwareCpu,
};
use hardware_interface_schema::Encode;
use pdma_model_contract::{
    CONTROL_CHANNEL_BASE, ControlRegisters, ControlResponse, ControlResponseStatus,
    ControlRingRegisters, Ring,
};
use pdma_model_host::{Management, firmware};
use std::{
    future::{Future, pending, poll_fn},
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
enum Moment {
    Before,
    After,
}

#[derive(Debug)]
struct Pause {
    address: u32,
    moment: Moment,
    armed: AtomicBool,
    hit: AtomicBool,
}

#[derive(Debug)]
struct PausingBus<B> {
    inner: B,
    pause: Arc<Pause>,
}

impl<B: RegisterAccess<Error = AccessError> + Sync> RegisterAccess for PausingBus<B> {
    type Error = AccessError;

    async fn read(&self, address: u32, out: &mut [u8]) -> Result<(), Self::Error> {
        self.inner.read(address, out).await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        if address == self.pause.address && self.pause.armed.swap(false, Ordering::SeqCst) {
            match self.pause.moment {
                Moment::Before => {}
                Moment::After => self.inner.write(address, bytes).await?,
            }
            self.pause.hit.store(true, Ordering::SeqCst);
            return pending().await;
        }
        self.inner.write(address, bytes).await
    }
}

async fn cursor(
    bus: &(impl RegisterAccess<Error = AccessError> + Sync),
    offset: usize,
) -> TestResult<u32> {
    let mut bytes = [0; 4];
    bus.read(CONTROL_CHANNEL_BASE + u32::try_from(offset)?, &mut bytes)
        .await?;
    Ok(u32::from_le_bytes(bytes))
}

async fn cancellation_case(response_ack: bool, moment: Moment) -> TestResult {
    // Given: real control hardware and firmware, with one Host write paused at its boundary.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let command_producer =
        ControlRegisters::COMMAND.offset() + ControlRingRegisters::PRODUCER.offset();
    let response_consumer =
        ControlRegisters::RESPONSE.offset() + ControlRingRegisters::CONSUMER.offset();
    let offset = if response_ack {
        response_consumer
    } else {
        command_producer
    };
    let pause = Arc::new(Pause {
        address: CONTROL_CHANNEL_BASE + u32::try_from(offset)?,
        moment,
        armed: AtomicBool::new(true),
        hit: AtomicBool::new(false),
    });
    let cpu = SoftwareCpu::new();
    let program = Box::pin(cpu.run_main(async {
        firmware::wait_ready(&host.bus, Duration::from_secs(1)).await?;
        let storage = rings(&allocator)?;
        let mut management = Management::setup(
            PausingBus {
                inner: host.bus.clone(),
                pause: Arc::clone(&pause),
            },
            memory.clone(),
            storage,
        )
        .await?;
        {
            let mut request = pin!(management.acquire_channel());
            poll_fn(|cx| {
                assert!(
                    request.as_mut().poll(cx).is_pending(),
                    "request completed before the injected boundary"
                );
                if pause.hit.load(Ordering::SeqCst) {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;
            // When: Host cancels its request at the selected publication boundary.
        }
        let committed = match moment {
            Moment::Before => 0,
            Moment::After => 1,
        };
        assert_eq!(cursor(&host.bus, offset).await?, committed);
        if response_ack && committed == 1 {
            let replacement = ControlResponse {
                id: 99,
                status: ControlResponseStatus::NoResources,
                channel: 7,
            };
            memory.write_typed(
                MemoryAddress::new(storage.response.entry(0)?),
                MemoryAlignment::ONE,
                &replacement.encode()?,
            )?;
        }
        let response = management.wait_pending().await?;
        // Then: resumption returns the original result and each actual cursor advanced once.
        assert_eq!(response.id, 1);
        assert_eq!(response.status, ControlResponseStatus::Success);
        assert_eq!(response.channel, 0);
        for offset in [
            command_producer,
            ControlRegisters::COMMAND.offset() + ControlRingRegisters::CONSUMER.offset(),
            ControlRegisters::RESPONSE.offset() + ControlRingRegisters::PRODUCER.offset(),
            response_consumer,
        ] {
            assert_eq!(cursor(&host.bus, offset).await?, 1);
        }
        Ok(())
    }));
    tokio::time::timeout(Duration::from_secs(3), system.run_until(program)).await??
}

#[tokio::test]
async fn publication_cancellation_before_command_producer() -> TestResult {
    cancellation_case(false, Moment::Before).await
}

#[tokio::test]
async fn publication_cancellation_after_command_producer() -> TestResult {
    cancellation_case(false, Moment::After).await
}

#[tokio::test]
async fn publication_cancellation_before_response_consumer() -> TestResult {
    cancellation_case(true, Moment::Before).await
}

#[tokio::test]
async fn publication_cancellation_after_response_consumer_uses_cached_result() -> TestResult {
    cancellation_case(true, Moment::After).await
}
