//! Shared-memory queue fixtures and a suspendable register transport.

use hardware_interface_runtime::{
    MemoryAccess, MemoryAlignment, RegisterAccess, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator,
};
use hardware_interface_schema::Decode;
use pdma_model_contract::control::{
    ControlQueue, ControlQueueRaw, ControlRequest, ControlResponse,
};
use pdma_model_host::{Management, ManagementConfig, ManagementRings, RingConfig};
use pdma_model_software::{
    control::{ControlQueueLayout, InterruptEvent},
    ring::SoftRing,
};
use std::{
    convert::Infallible,
    future::{Future, poll_fn},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Poll,
};

#[derive(Debug, Default)]
pub(super) struct Bus {
    pub blocked: AtomicBool,
    pub kicks: AtomicUsize,
}

impl RegisterAccess for &Bus {
    type Error = Infallible;
    async fn read(&self, _: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        bytes.fill(0);
        Ok(())
    }
    async fn write(&self, _: u32, _: &[u8]) -> Result<(), Self::Error> {
        if self.blocked.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        self.kicks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

pub(super) struct Fixture {
    pub bus: Bus,
    pub memory: SharedMemoryAccess,
    pub rings: ManagementRings,
    pub event: InterruptEvent,
}

impl Fixture {
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
        let alignment = MemoryAlignment::try_new(8)?;
        Ok(Self {
            bus: Bus::default(),
            memory: allocator.memory_access(),
            rings: ManagementRings {
                state: allocator.allocate(u64::try_from(ControlQueue::RECORD_SIZE)?, alignment)?,
                command: RingConfig::new(allocator.allocate(128, alignment)?.raw(), 2)?,
                response: RingConfig::new(allocator.allocate(128, alignment)?.raw(), 2)?,
                event: RingConfig::new(allocator.allocate(16, alignment)?.raw(), 2)?,
            },
            event: InterruptEvent::new(),
        })
    }

    pub(super) async fn setup(
        &self,
    ) -> Result<Management<&Bus, SharedMemoryAccess>, pdma_model_host::Error<Infallible>> {
        Management::setup(
            &self.bus,
            self.memory.clone(),
            ManagementConfig {
                addresses: pdma_model_software::control::ManagementAddresses {
                    dma: 0,
                    windows: 0x2000,
                    control: 0x3000,
                    firmware: 0x4000,
                    contexts: None,
                },
                rings: self.rings,
                interrupt: self.event.clone(),
                event_interrupt: InterruptEvent::new(),
            },
        )
        .await
    }

    pub(super) fn state(&self) -> Result<ControlQueue, Box<dyn std::error::Error>> {
        Ok(self
            .memory
            .read_typed::<ControlQueueRaw>(self.rings.state, MemoryAlignment::ONE)?
            .decode()?)
    }

    pub(super) async fn take(
        &self,
        ring: &mut SoftRing<ControlRequest>,
    ) -> Result<ControlRequest, Box<dyn std::error::Error>> {
        let layout = ControlQueueLayout::new(self.rings.state)?;
        let io = layout.command_io(&self.memory);
        let request = ring.peek(&io).await?.ok_or("empty command ring")?;
        ring.consume(&io).await?;
        Ok(request)
    }

    pub(super) async fn respond(
        &self,
        ring: &mut SoftRing<ControlResponse>,
        response: ControlResponse,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let layout = ControlQueueLayout::new(self.rings.state)?;
        assert!(
            ring.try_push(&layout.response_io(&self.memory), response)
                .await?
        );
        Ok(())
    }
}

pub(super) async fn pending_once(future: impl Future) {
    let mut future = std::pin::pin!(future);
    poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
}

pub(super) const fn response(id: u64, channel: u32) -> ControlResponse {
    ControlResponse {
        context: 0,
        id,
        channel,
        status: pdma_model_contract::control::ControlResponseStatus::Success,
    }
}
