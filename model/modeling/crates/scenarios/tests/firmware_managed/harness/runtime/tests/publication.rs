use super::{
    AccessError, Managed, client,
    control::{memory::QueueMemory, rings},
};
use crate::harness::TestResult;
use crate::harness::runtime::platform::CONTROL_CHANNEL_BASE;
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::memory::{SharedMemory, SharedMemoryAllocator};
use hardware_interface_io::register::RegisterAccess;
use hardware_interface_schema::Encode;
use pdma_model_contract::control::{ControlResponse, ControlResponseStatus};
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::memory::FunctionId;
use pdma_model_contract::ring::Ring;
use pdma_model_host::{Management, firmware};
use std::{
    future::{Future, pending, poll_fn},
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    remaining: AtomicUsize,
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
        if address == self.pause.address
            && self
                .pause
                .remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .ok()
                == Some(1)
        {
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

async fn cancellation_case(response_ack: bool, moment: Moment) -> TestResult {
    // Given: real control hardware and firmware, with one Host write paused at its boundary.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let pause = Arc::new(Pause {
        address: CONTROL_CHANNEL_BASE + u32::try_from(ControlRegisters::REQUEST.offset())?,
        moment,
        remaining: AtomicUsize::new(0),
        hit: AtomicBool::new(false),
    });
    let program = Box::pin(client::run(host.runtime, async |interrupt| {
        firmware::wait_ready(
            &host.bus,
            host.bus.addresses().firmware,
            Duration::from_secs(1),
        )
        .await?;
        let storage = rings(&allocator)?;
        let mut management = Management::setup(
            PausingBus {
                inner: host.bus.clone(),
                pause: Arc::clone(&pause),
            },
            memory.clone(),
            client::config(storage, &interrupt),
        )
        .await?;
        pause
            .remaining
            .store(if response_ack { 2 } else { 1 }, Ordering::SeqCst);
        let queue = QueueMemory::new(&memory, storage.state);
        {
            let mut request = pin!(management.acquire_channel(FunctionId::new(0)));
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
        assert_eq!(queue.read()?.command.producer, 1);
        assert_eq!(queue.read()?.response.consumer, u32::from(response_ack));
        if response_ack {
            let replacement = ControlResponse {
                id: 99,
                context: 0,
                status: ControlResponseStatus::NoResources,
                channel: 7,
            };
            memory.write_buffer(
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
        let state = queue.read()?;
        assert_eq!(state.command.producer, 1);
        assert_eq!(state.command.consumer, 1);
        assert_eq!(state.response.producer, 1);
        assert_eq!(state.response.consumer, 1);
        Ok(())
    }));
    tokio::time::timeout(Duration::from_secs(3), system.run_until(program)).await??
}

#[tokio::test]
async fn publication_cancellation_before_command_notification() -> TestResult {
    cancellation_case(false, Moment::Before).await
}

#[tokio::test]
async fn publication_cancellation_after_command_notification() -> TestResult {
    cancellation_case(false, Moment::After).await
}

#[tokio::test]
async fn publication_cancellation_before_response_space_notification() -> TestResult {
    cancellation_case(true, Moment::Before).await
}

#[tokio::test]
async fn publication_cancellation_after_response_space_notification_uses_cached_result()
-> TestResult {
    cancellation_case(true, Moment::After).await
}
