use super::*;
use crate::harness::runtime::{memory::SpaceError, platform::MemoryRequest};
use hardware_interface_io::memory::AtomicReadOrder;
use hardware_interface_io::memory::MemoryAccessError;

#[derive(Clone)]
struct Tagged {
    memory: SharedMemory,
    request: Option<MemoryRequest>,
    reports: Arc<Mutex<Vec<(ChannelId, MemoryIdentity)>>>,
}

impl Tagged {
    fn bind(&self, request: MemoryRequest) -> Self {
        Self {
            request: Some(request),
            ..self.clone()
        }
    }

    fn authorize(&self) -> Result<(), AtomicAccessError<MemoryAccessError>> {
        let request = self.request.expect("atomic request must be bound");
        let channel = request.channel.expect("DMA source must be retained");
        self.reports
            .lock()
            .unwrap()
            .push((channel, request.identity));
        if channel == ChannelId::new(1) {
            return Err(AtomicAccessError::Access(MemoryAccessError::Unmapped {
                address: ADDRESS,
            }));
        }
        Ok(())
    }
}

impl MemoryAccess for Tagged {
    type Error = MemoryAccessError;
    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error> {
        self.memory.validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.memory.read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        self.memory.write(address, alignment, bytes)
    }
    fn atomic32_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u32>,
    ) -> Result<u32, AtomicAccessError<Self::Error>> {
        self.authorize()?;
        self.memory.atomic32(address, operation)
    }
    fn atomic64_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        operation: AtomicOperation<u64>,
    ) -> Result<u64, AtomicAccessError<Self::Error>> {
        self.authorize()?;
        self.memory.atomic64(address, operation)
    }
}

#[test]
fn atomic_binding_retains_source_tags_and_failure_before_modification() {
    // Given: two channels share one Function and backing but the platform rejects the second source.
    let backing = space(17);
    let reports = Arc::new(Mutex::new(Vec::new()));
    let identity = MemoryIdentity {
        function: FunctionId::new(0),
        attributes: ContextAttributes::DEFAULT,
    };
    let backend = Tagged {
        memory: backing.clone(),
        request: None,
        reports: Arc::clone(&reports),
    };
    let spaces = FunctionSpaces {
        backing: vec![(identity, backend)],
        bind: Some(Tagged::bind),
    };
    let windows = ChannelWindows::new(2, 0, u32::try_from(BASE).unwrap()).unwrap();
    let mut table = ContextTable::new(1, 2, 0).unwrap();
    let mut dma = Dma::new(SharedMemory::new(), 2).unwrap();
    let entry = ContextTableRegisters::for_channels(1, 2)
        .unwrap()
        .entries()
        .offset(0)
        .unwrap();
    table
        .management_write(
            (
                entry + ContextEntryRegisters::VALID.offset(),
                &1_u32.to_le_bytes(),
            ),
            (&mut dma, &windows),
        )
        .unwrap();
    let memory = RoutedMemory::with_contexts(
        Arc::new(Mutex::new(windows)),
        Arc::new(Mutex::new(table)),
        spaces,
    );
    let failing = DmaAccess {
        channel: ChannelId::new(1),
        context: ContextId::new(0),
    };
    // When: both widths encounter a source-specific fault before the accepted source performs a load.
    let failed32 = memory.atomic32_with_sideband(
        &failing,
        ADDRESS,
        AtomicOperation::Store {
            value: 99,
            order: AtomicWriteOrder::Release,
        },
    );
    let failed64 = memory.atomic64_with_sideband(
        &failing,
        ADDRESS,
        AtomicOperation::Store {
            value: 99,
            order: AtomicWriteOrder::Release,
        },
    );
    let loaded = memory.atomic64_with_sideband(
        &access(0),
        ADDRESS,
        AtomicOperation::Load(AtomicReadOrder::Acquire),
    );
    // Then: error details and old bits survive, with distinct source association on every call.
    let error = || {
        AtomicAccessError::Access(RouteError::Context(MemoryError::Downstream(
            SpaceError::Downstream(MemoryAccessError::Unmapped { address: ADDRESS }),
        )))
    };
    assert_eq!(failed32, Err(error()));
    assert_eq!(failed64, Err(error()));
    assert_eq!(loaded, Ok(17));
    assert_eq!(
        backing
            .read_buffer::<[u8; 8]>(ADDRESS, MemoryAlignment::ONE)
            .unwrap(),
        17_u64.to_le_bytes()
    );
    assert_eq!(
        *reports.lock().unwrap(),
        vec![
            (ChannelId::new(1), identity),
            (ChannelId::new(1), identity),
            (ChannelId::new(0), identity)
        ]
    );
}
