use super::super::{HostIrq, HostVector};
use super::{
    Managed, client,
    control::{
        memory::{QueueMemory, kick, poll_once},
        rings,
    },
};
use crate::harness::TestResult;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_io::memory::{SharedMemory, SharedMemoryAllocator};
use hw_model_irq::InterruptHandler;
use hw_model_schema::{Decode, Encode};
use pdma_model_contract::control::{
    ControlRequest, ControlRequestOpcode, ControlResponseRaw, ControlResponseStatus,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;
use pdma_model_contract::ring::Ring;
use pdma_model_host::Management;
use pdma_model_software::control::{ControlIsr, InterruptEvent};
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
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let mut running = pin!(system.run());
    assert!(poll_once(running.as_mut()).await.is_pending());
    let program = client::run(host.runtime, async |interrupt| {
        let storage = rings(&allocator)?;
        let _client = Management::setup(
            host.bus.clone(),
            memory.clone(),
            client::config(storage, &interrupt),
        )
        .await?;
        let queue = QueueMemory::new(&memory, storage.state);
        assert!(poll_once(running.as_mut()).await.is_pending());
        // When: Host publishes the complete record and cursor without a doorbell.
        memory.write_buffer(
            MemoryAddress::new(storage.command.entry(0)?),
            MemoryAlignment::ONE,
            &ControlRequest {
                id: 17,
                context: 0,
                attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
                function: 0,
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
            result = queue.wait_response(&interrupt.response, 1) => result?,
        }
        let response = memory
            .read_buffer::<ControlResponseRaw>(
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

struct Handler {
    control: ControlIsr,
    dma: Arc<AtomicUsize>,
    management: Arc<AtomicUsize>,
}

#[tokio::test]
async fn spurious_request_doorbells_leave_empty_queues_and_resources_unchanged() -> TestResult {
    // Given: firmware attached to empty control queues through the real interrupt path.
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let program = client::run(host.runtime, async |interrupt| {
        let storage = rings(&allocator)?;
        let mut management = Management::setup(
            host.bus.clone(),
            memory.clone(),
            client::config(storage, &interrupt),
        )
        .await?;
        // When: repeated doorbells arrive without any published request.
        for _ in 0..3 {
            kick(&host.bus).await?;
        }
        tokio::task::yield_now().await;
        let queue = QueueMemory::new(&memory, storage.state);
        assert_eq!(queue.read()?.command.consumer, 0);
        assert_eq!(queue.read()?.response.producer, 0);
        // Then: the first real acquisition still obtains the only channel.
        assert_eq!(
            management.acquire_channel(FunctionId::new(0)).await?,
            ChannelId::new(0)
        );
        management
            .release_channel(FunctionId::new(0), ChannelId::new(0))
            .await?;
        Ok(())
    });
    tokio::time::timeout(Duration::from_secs(3), system.run_until(program)).await??
}

impl InterruptHandler<HostIrq> for Handler {
    type Error = Infallible;

    async fn handle(&mut self, irq: &HostIrq) -> Result<(), Self::Error> {
        match irq {
            HostIrq::Dma(_) => {
                self.dma.fetch_add(1, Ordering::SeqCst);
            }
            HostIrq::Event(_) => panic!("unexpected event in healthy control test"),
            HostIrq::Control(irq) => {
                self.management.fetch_add(1, Ordering::SeqCst);
                self.control.handle(irq).await?;
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn response_notifications_resume_host_without_dma_interrupt_dispatch() -> TestResult {
    // Given: both vectors registered on a real Host interrupt runtime.
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
    let memory = allocator.memory_access();
    let (system, host) = Managed::new(memory.clone())?;
    let dma = Arc::new(AtomicUsize::new(0));
    let management = Arc::new(AtomicUsize::new(0));
    let program = host.runtime.run_main::<Handler, _>(async |registration| {
        let interrupt = InterruptEvent::new();
        for vector in [HostVector::Control, HostVector::Dma] {
            registration.register(
                vector,
                Handler {
                    control: interrupt.handler(),
                    dma: Arc::clone(&dma),
                    management: Arc::clone(&management),
                },
            )?;
        }
        let mut client = Management::setup(
            host.bus,
            memory,
            client::config(
                rings(&allocator)?,
                &client::Interrupts {
                    response: interrupt,
                    event: InterruptEvent::new(),
                },
            ),
        )
        .await?;
        // When: firmware completes management requests without any DMA command.
        let channel = client.acquire_channel(FunctionId::new(0)).await?;
        client.release_channel(FunctionId::new(0), channel).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(channel)
    });
    let (channel, interrupts) =
        tokio::time::timeout(Duration::from_secs(3), system.run_until(Box::pin(program)))
            .await???;
    // Then: the management ISR resumes Host and DMA dispatch remains idle.
    assert_eq!(channel?, ChannelId::new(0));
    assert!(management.load(Ordering::SeqCst) > 0);
    assert_eq!(dma.load(Ordering::SeqCst), 0);
    assert_eq!(interrupts.deliveries(), management.load(Ordering::SeqCst));
    Ok(())
}
