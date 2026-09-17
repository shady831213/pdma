//! Management bootstrap publishes shared state before its first doorbell.

use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, NotificationWait, RegisterAccess, SharedMemory,
    SharedMemoryAccess, SharedMemoryAllocator, SoftwareCpu,
};
use hardware_interface_schema::Decode;
use pdma_model_contract::{CONTROL_CHANNEL_BASE, ControlQueue, ControlQueueRaw, ControlRegisters};
use pdma_model_host::{Management, ManagementRings, RingConfig};
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
impl NotificationWait for &Bus {
    async fn wait_notification(&self) {
        std::future::pending::<()>().await;
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
                ManagementRings {
                    state,
                    command: RingConfig::new(command.raw(), 2).unwrap(),
                    response: RingConfig::new(response.raw(), 2).unwrap(),
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
            ManagementRings {
                state,
                command: RingConfig::new(0x2000, 2).unwrap(),
                response: RingConfig::new(0x3000, 2).unwrap(),
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
