use super::{
    AccessError, Managed,
    control::{
        memory::{QueueMemory, kick, poll_once},
        rings,
    },
};
use crate::harness::TestResult;
use hardware_interface_runtime::{
    InterruptHandler, MemoryAccess, MemoryAddress, MemoryAlignment, NotificationWait,
    RegisterAccess, SharedMemory, SharedMemoryAllocator, SoftwareCpu,
};
use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::{
    ChannelId, ControlRequest, ControlRequestOpcode, ControlResponseRaw, ControlResponseStatus,
    Ring,
};
use pdma_model_host::{Management, runtime::Irq};
use std::{
    convert::Infallible,
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[tokio::test]
async fn memory_publication_waits_for_request_doorbell_before_firmware_consumes() -> TestResult {
    // Given: firmware is parked after attaching empty control queues.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let mut running = pin!(system.run());
    assert!(poll_once(running.as_mut()).await.is_pending());
    let cpu = SoftwareCpu::new();
    let program = cpu.run_main(async {
        let storage = rings(&allocator)?;
        let _client = Management::setup(host.bus.clone(), memory.clone(), storage).await?;
        let queue = QueueMemory::new(&memory, storage.state);
        assert!(poll_once(running.as_mut()).await.is_pending());
        // When: Host publishes the complete record and cursor without a doorbell.
        memory.write_typed(
            MemoryAddress::new(storage.command.entry(0)?),
            MemoryAlignment::ONE,
            &ControlRequest {
                id: 17,
                opcode: ControlRequestOpcode::AcquireChannel,
                channel: 0,
            }
            .encode()?,
        )?;
        queue.publish_command(1)?;
        assert!(poll_once(running.as_mut()).await.is_pending());
        // Then: parked firmware leaves it untouched until the request notification arrives.
        assert_eq!(queue.read()?.command.consumer, 0);
        assert_eq!(queue.read()?.response.producer, 0);
        kick(&host.bus).await?;
        tokio::select! {
            result = running.as_mut() => panic!("system stopped: {result:?}"),
            result = queue.wait_response(&host.bus, 1) => result?,
        }
        let response = memory
            .read_typed::<ControlResponseRaw>(
                MemoryAddress::new(storage.response.entry(0)?),
                MemoryAlignment::ONE,
            )?
            .decode()?;
        assert_eq!(
            (response.id, response.status, response.channel),
            (17, ControlResponseStatus::Success, 0)
        );
        assert_eq!(queue.read()?.command.consumer, 1);
        Ok(())
    });
    tokio::time::timeout(Duration::from_secs(3), program).await?
}

struct ObservedWait<B> {
    inner: B,
    waits: Arc<AtomicUsize>,
}

impl<B: RegisterAccess<Error = AccessError> + Sync> RegisterAccess for ObservedWait<B> {
    type Error = AccessError;

    async fn read(&self, address: u32, out: &mut [u8]) -> Result<(), Self::Error> {
        self.inner.read(address, out).await
    }

    async fn write(&self, address: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.inner.write(address, bytes).await
    }
}

impl<B: NotificationWait> NotificationWait for ObservedWait<B> {
    async fn wait_notification(&self) {
        self.waits.fetch_add(1, Ordering::SeqCst);
        self.inner.wait_notification().await;
    }
}

struct Handler;

impl InterruptHandler<Irq> for Handler {
    type Error = Infallible;

    async fn handle(&mut self, _: &Irq) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn response_notifications_resume_host_without_dma_interrupt_dispatch() -> TestResult {
    // Given: a real Host interrupt runtime and a separately observed management wait.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let waits = Arc::new(AtomicUsize::new(0));
    let program = host.runtime.run_main::<Handler, _>(async |registration| {
        registration.register((), Handler)?;
        let mut management = Management::setup(
            ObservedWait {
                inner: host.bus,
                waits: Arc::clone(&waits),
            },
            memory,
            rings(&allocator)?,
        )
        .await?;
        // When: firmware completes management requests without any DMA command.
        let channel = management.acquire_channel().await?;
        management.release_channel(channel).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(channel)
    });
    let (channel, interrupts) =
        tokio::time::timeout(Duration::from_secs(3), system.run_until(Box::pin(program)))
            .await???;
    // Then: Host notification waits resumed successfully and IRQ dispatch remained idle.
    assert_eq!(channel?, ChannelId::new(0));
    assert!(waits.load(Ordering::SeqCst) > 0);
    assert_eq!(interrupts.deliveries(), 0);
    Ok(())
}
