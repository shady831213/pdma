//! Management bootstrap publishes shared state before its first doorbell.
const CONTROL_CHANNEL_BASE: u32 = 0x3000;

use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator, SoftwareCpu,
};
use hardware_interface_schema::Decode;
use pdma_model_contract::control::{ControlQueue, ControlQueueRaw};
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::{Management, ManagementConfig, ManagementRings, RingConfig};
use pdma_model_software::control::InterruptEvent;
use std::{
    io,
    sync::{Arc, Mutex},
};

struct Bus {
    memory: SharedMemoryAccess,
    state: MemoryAddress,
    writes: Mutex<Vec<u32>>,
    enabled: bool,
}

impl RegisterAccess for &Bus {
    type Error = io::Error;
    async fn read(&self, address: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        assert_eq!(
            address,
            CONTROL_CHANNEL_BASE
                + u32::try_from(ControlRegisters::ENABLE.offset()).map_err(io::Error::other)?
        );
        bytes.copy_from_slice(&u32::from(self.enabled).to_le_bytes());
        Ok(())
    }
    async fn write(&self, address: u32, _: &[u8]) -> Result<(), Self::Error> {
        let state = self
            .memory
            .read_typed::<ControlQueueRaw>(self.state, MemoryAlignment::ONE)
            .map_err(io::Error::other)?
            .decode()
            .map_err(io::Error::other)?;
        assert_eq!(state.command.capacity, 2);
        assert_eq!(state.response.capacity, 2);
        assert_eq!(state.command.producer, 0);
        assert_eq!(state.response.consumer, 0);
        self.writes
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?
            .push(address);
        Ok(())
    }
}

#[tokio::test]
async fn descriptor_is_in_memory_before_bootstrap_enable_and_request_doorbell() {
    // Given: software allocated the queue descriptor and entry buffers.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let state = allocator
        .allocate(
            ControlQueue::RECORD_SIZE as u64,
            MemoryAlignment::try_new(8).unwrap(),
        )
        .unwrap();
    let command = allocator
        .allocate(128, MemoryAlignment::try_new(8).unwrap())
        .unwrap();
    let response = allocator
        .allocate(128, MemoryAlignment::try_new(8).unwrap())
        .unwrap();
    let bus = Bus {
        memory: allocator.memory_access(),
        state,
        writes: Mutex::new(Vec::new()),
        enabled: false,
    };
    // When: the Host sets up its management endpoint on its software CPU.
    SoftwareCpu::new()
        .run_main(async {
            let _client = Management::setup(
                &bus,
                allocator.memory_access(),
                ManagementConfig {
                    addresses: pdma_model_software::control::ManagementAddresses {
                        dma: 0,
                        windows: 0x2000,
                        control: 0x3000,
                        firmware: 0x4000,
                        contexts: None,
                    },
                    rings: ManagementRings {
                        state,
                        command: RingConfig::new(command.raw(), 2).unwrap(),
                        response: RingConfig::new(response.raw(), 2).unwrap(),
                        event: RingConfig::new(0x6000, 2).unwrap(),
                    },
                    interrupt: InterruptEvent::new(),
                    event_interrupt: InterruptEvent::new(),
                },
            )
            .await
            .unwrap();
        })
        .await;
    // Then: bootstrap and enable precede the request notification, after memory publication.
    assert_eq!(
        *bus.writes.lock().unwrap(),
        [
            ControlRegisters::ATTRIBUTES.offset()
                + pdma_model_contract::hw_components::context::ContextAttributeRegisters::ADDRESS_SPACE.offset(),
            ControlRegisters::ATTRIBUTES.offset()
                + pdma_model_contract::hw_components::context::ContextAttributeRegisters::PASID.offset(),
            ControlRegisters::BOOTSTRAP.offset(),
            ControlRegisters::ENABLE.offset(),
            ControlRegisters::REQUEST.offset()
        ]
        .map(|offset| CONTROL_CHANNEL_BASE + u32::try_from(offset).unwrap())
    );
}

#[tokio::test]
async fn repeated_setup_cannot_reinitialize_live_shared_cursors() {
    // Given: an enabled control queue with existing shared state.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let state = allocator
        .allocate(
            ControlQueue::RECORD_SIZE as u64,
            MemoryAlignment::try_new(8).unwrap(),
        )
        .unwrap();
    let memory = allocator.memory_access();
    let original = [0xa5; ControlQueue::RECORD_SIZE];
    memory
        .write(state, MemoryAlignment::ONE, &original)
        .unwrap();
    let bus = Bus {
        memory: memory.clone(),
        state,
        writes: Mutex::new(Vec::new()),
        enabled: true,
    };
    // When: software attempts another setup against the live channel.
    let result = SoftwareCpu::new()
        .run_main(Management::setup(
            &bus,
            memory.clone(),
            ManagementConfig {
                addresses: pdma_model_software::control::ManagementAddresses {
                    dma: 0,
                    windows: 0x2000,
                    control: 0x3000,
                    firmware: 0x4000,
                    contexts: None,
                },
                rings: ManagementRings {
                    state,
                    command: RingConfig::new(0x2000, 2).unwrap(),
                    response: RingConfig::new(0x3000, 2).unwrap(),
                    event: RingConfig::new(0x6000, 2).unwrap(),
                },
                interrupt: InterruptEvent::new(),
                event_interrupt: InterruptEvent::new(),
            },
        ))
        .await;
    // Then: it is rejected before memory or bootstrap is changed.
    assert!(matches!(
        result,
        Err(pdma_model_host::Error::ManagementBusy)
    ));
    let mut observed = [0; ControlQueue::RECORD_SIZE];
    memory
        .read(state, MemoryAlignment::ONE, &mut observed)
        .unwrap();
    assert_eq!(observed, original);
    assert!(bus.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn nonempty_or_invalid_control_cursors_reject_a_new_request()
-> Result<(), Box<dyn std::error::Error>> {
    use pdma_model_contract::control::ControlRingState;
    use pdma_model_host::Error;
    // Given: initialized cursors with unmapped entry storage and no outstanding request.
    let allocator = SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into());
    let state = allocator.allocate(
        u64::try_from(ControlQueue::RECORD_SIZE)?,
        MemoryAlignment::try_new(8)?,
    )?;
    let memory = allocator.memory_access();
    let bus = Bus {
        memory: memory.clone(),
        state,
        writes: Mutex::new(Vec::new()),
        enabled: false,
    };
    SoftwareCpu::new()
        .run_main(async {
            let mut client = Management::setup(
                &bus,
                memory.clone(),
                ManagementConfig {
                    addresses: pdma_model_software::control::ManagementAddresses {
                        dma: 0,
                        windows: 0x2000,
                        control: 0x3000,
                        firmware: 0x4000,
                        contexts: None,
                    },
                    rings: ManagementRings {
                        state,
                        command: RingConfig::new(0x2000, 2)?,
                        response: RingConfig::new(0x3000, 2)?,
                        event: RingConfig::new(0x6000, 2).unwrap(),
                    },
                    interrupt: InterruptEvent::new(),
                    event_interrupt: InterruptEvent::new(),
                },
            )
            .await?;
            let writes = bus.writes.lock().unwrap().len();
            for offset in [ControlQueue::COMMAND_OFFSET, ControlQueue::RESPONSE_OFFSET] {
                let producer = MemoryAddress::new(
                    state.raw() + u64::try_from(offset + ControlRingState::PRODUCER_OFFSET)?,
                );
                let consumer = MemoryAddress::new(
                    state.raw() + u64::try_from(offset + ControlRingState::CONSUMER_OFFSET)?,
                );
                for (p, c) in [(1_u32, 0_u32), (3, 0), (0, 1)] {
                    memory.write(producer, MemoryAlignment::ONE, &p.to_le_bytes())?;
                    memory.write(consumer, MemoryAlignment::ONE, &c.to_le_bytes())?;
                    // When: starting a request with a nonempty or malformed ring.
                    let result = client.acquire_channel(FunctionId::new(0)).await;
                    // Then: classification remains a protocol error, with no publication or pending request.
                    assert!(
                        matches!(result, Err(Error::ManagementProtocol)),
                        "{result:?}"
                    );
                    assert!(matches!(
                        client.wait_pending().await,
                        Err(Error::ManagementIdle)
                    ));
                    assert_eq!(bus.writes.lock().unwrap().len(), writes);
                }
                memory.write(producer, MemoryAlignment::ONE, &0_u32.to_le_bytes())?;
                memory.write(consumer, MemoryAlignment::ONE, &0_u32.to_le_bytes())?;
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await
}
