use super::support::{Fixture, response};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
    SoftwareCpu,
};
use pdma_model_contract::control::{ControlQueue, ControlRingState};
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::{Error, Management, ManagementConfig};
use pdma_model_software::ring::SoftRing;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone)]
struct FallibleMemory {
    memory: SharedMemoryAccess,
    rejected: Arc<AtomicU64>,
}

#[derive(Debug, PartialEq, Eq)]
enum BackendError {
    Rejected { address: MemoryAddress, reason: u32 },
    Backing(MemoryAccessError),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BackendError {}

impl MemoryAccess for FallibleMemory {
    type Error = BackendError;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), BackendError> {
        self.memory
            .validate(address, alignment, length)
            .map_err(BackendError::Backing)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), BackendError> {
        self.memory
            .read(address, alignment, bytes)
            .map_err(BackendError::Backing)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), BackendError> {
        if self
            .rejected
            .compare_exchange(address.raw(), 0, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return Err(BackendError::Rejected {
                address,
                reason: 73,
            });
        }
        self.memory
            .write(address, alignment, bytes)
            .map_err(BackendError::Backing)
    }
}

#[tokio::test]
async fn publication_and_consumption_memory_failures_preserve_original_work() {
    let fixture = Fixture::new().unwrap();
    SoftwareCpu::new()
        .run_main(async {
            // Given: a memory backend that can reject one selected cursor write.
            let memory = FallibleMemory {
                memory: fixture.memory.clone(),
                rejected: Arc::new(AtomicU64::new(0)),
            };
            let mut client = Management::setup(
                &fixture.bus,
                memory.clone(),
                ManagementConfig {
                    addresses: pdma_model_software::control::ManagementAddresses {
                        dma: 0,
                        windows: 0x2000,
                        control: 0x3000,
                        firmware: 0x4000,
                        contexts: None,
                    },
                    rings: fixture.rings,
                    interrupt: fixture.event.clone(),
                    event_interrupt: pdma_model_software::control::InterruptEvent::new(),
                },
            )
            .await
            .unwrap();
            memory.rejected.store(
                fixture.rings.state.raw()
                    + u64::try_from(
                        ControlQueue::COMMAND_OFFSET + ControlRingState::PRODUCER_OFFSET,
                    )
                    .unwrap(),
                Ordering::SeqCst,
            );
            // When: producer publication fails and is explicitly resumed.
            let rejected = memory.rejected.load(Ordering::SeqCst);
            let error = client.submit_acquire(FunctionId::new(0)).await.unwrap_err();
            assert!(std::error::Error::source(&error).is_some());
            assert!(matches!(error, Error::Memory(BackendError::Rejected { address, reason: 73 }) if address.raw() == rejected));
            assert_eq!(fixture.state().unwrap().command.producer, 0);
            assert!(matches!(
                client.submit_acquire(FunctionId::new(0)).await,
                Err(Error::ManagementBusy)
            ));
            let id = client.resume_submission().await.unwrap();
            // Then: the original command is published exactly once with its retained ID.
            let mut commands = SoftRing::new(fixture.rings.command);
            assert_eq!(fixture.take(&mut commands).await.unwrap().id, id.raw());
            assert_eq!(fixture.state().unwrap().command.producer, 1);
            let mut responses = SoftRing::new(fixture.rings.response);
            fixture
                .respond(&mut responses, response(id.raw(), 0))
                .await
                .unwrap();
            memory.rejected.store(
                fixture.rings.state.raw()
                    + u64::try_from(
                        ControlQueue::RESPONSE_OFFSET + ControlRingState::CONSUMER_OFFSET,
                    )
                    .unwrap(),
                Ordering::SeqCst,
            );
            assert!(matches!(client.try_response().await, Err(Error::Memory(_))));
            assert_eq!(fixture.state().unwrap().response.consumer, 0);
            assert_eq!(client.next_response().await.unwrap(), response(id.raw(), 0));
            assert_eq!(fixture.state().unwrap().response.consumer, 1);
        })
        .await;
}
