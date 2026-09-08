use crate::test_support::{self, TestQueue};
use crate::{DeviceCore, HardwareStepStatus, PdmaMemoryMappings, PdmaMemoryProvisioning};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
};
use pdma_model_contract::abi::registers::QueueRegisters;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadStridedCopyValues, AbiCommandPayloadValues, AbiEventAccess,
    AbiEventKind, AbiEventStage, QueueRingLayouts, RingLayout,
};
use pdma_model_contract::semantic::ResourceConfig;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Read,
    Write,
}

#[derive(Debug)]
struct FailingMemory {
    storage: Arc<SharedMemory>,
    failure: Failure,
    failed_address: u64,
    first_destination: u64,
    successful_writes: AtomicUsize,
}

impl MemoryAccess for FailingMemory {
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        if self.failure == Failure::Read && address.raw() == self.failed_address {
            return Err(MemoryAccessError::Unmapped { address });
        }
        self.storage.read(address, alignment, bytes)
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        if self.failure == Failure::Write && address.raw() == self.failed_address {
            return Err(MemoryAccessError::Unmapped { address });
        }
        self.storage.write(address, alignment, bytes)?;
        if address.raw() == self.first_destination {
            self.successful_writes.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[test]
fn execution_read_failure_retires_without_replaying_written_rows() {
    retires_once(Failure::Read);
}

#[test]
fn execution_write_failure_retires_without_replaying_written_rows() {
    retires_once(Failure::Write);
}

fn retires_once(failure: Failure) {
    // Given: registered rows whose backing store fails only on the second data access.
    let storage = Arc::new(SharedMemory::new());
    let mut provisioning = PdmaMemoryProvisioning::shared(storage.clone());
    let mut mappings = PdmaMemoryMappings::new(storage.clone());
    let stage = provisioning
        .stage(&mappings, &[8, 8, 128, 64, 128])
        .expect("regions");
    let regions = provisioning
        .commit(&mut mappings, stage)
        .expect("backing bytes");
    let [source, destination, sq, cq, eq] = regions.as_slice() else {
        panic!("five regions");
    };
    mappings
        .write_memory(source.base(), &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("source bytes");
    let failed_address = match failure {
        Failure::Read => source.base().raw() + 4,
        Failure::Write => destination.base().raw() + 4,
    };
    let backend = Arc::new(FailingMemory {
        storage,
        failure,
        failed_address,
        first_destination: destination.base().raw(),
        successful_writes: AtomicUsize::new(0),
    });
    let mut core = DeviceCore::new(backend.clone(), ResourceConfig::default());
    core.memory
        .register_regions(&regions)
        .expect("hardware mappings");
    let queue = TestQueue {
        index: 0,
        layouts: QueueRingLayouts::new(
            RingLayout::try_new(sq.region(), 2).expect("SQ"),
            RingLayout::try_new(cq.region(), 2).expect("CQ"),
            RingLayout::try_new(eq.region(), 2).expect("EQ"),
        ),
    };
    for (offset, value) in [
        (QueueRegisters::SQ_BASE.offset(), sq.base().raw()),
        (QueueRegisters::SQ_DEPTH.offset(), 2),
        (QueueRegisters::CQ_BASE.offset(), cq.base().raw()),
        (QueueRegisters::CQ_DEPTH.offset(), 2),
        (QueueRegisters::EQ_BASE.offset(), eq.base().raw()),
        (QueueRegisters::EQ_DEPTH.offset(), 2),
        (QueueRegisters::ENABLE.offset(), 1),
    ] {
        test_support::write_queue(&mut core, queue, offset, value);
    }
    test_support::submit(
        &mut core,
        queue,
        AbiCommand::from_payload(
            23,
            AbiCommandPayloadValues::StridedCopy(AbiCommandPayloadStridedCopyValues {
                source_dma_address: source.base().raw(),
                destination_dma_address: destination.base().raw(),
                rows: 2.into(),
                row_bytes: 4.into(),
                source_stride: 4.into(),
                destination_stride: 4.into(),
            }),
        ),
    );
    test_support::doorbell(&mut core, queue);
    // When: execution reaches the failing second row and retirement publishes its fault.
    assert!(core.step().is_err());
    // Then: SQ is consumed, EQ preserves the precise access site, and no write is replayed.
    assert_eq!(
        test_support::read_queue(&core, queue, QueueRegisters::SQ_CONSUMER.offset()),
        1
    );
    assert!(test_support::poll_completion(&mut core, queue).is_none());
    let event = test_support::poll_event(&mut core, queue).expect("execution event");
    let (stage, access) = match failure {
        Failure::Read => (AbiEventStage::Source, AbiEventAccess::Read),
        Failure::Write => (AbiEventStage::Destination, AbiEventAccess::Write),
    };
    assert_eq!(
        event.fault().tuple(),
        (
            AbiEventKind::UnmappedDmaAddress,
            failed_address,
            4,
            0,
            stage.raw(),
            access.raw(),
            1
        )
    );
    assert_eq!(core.step(), Ok(HardwareStepStatus::Idle));
    assert_eq!(backend.successful_writes.load(Ordering::SeqCst), 1);
    assert!(test_support::poll_event(&mut core, queue).is_none());
    assert_eq!(
        core.memory
            .read_memory(destination.base(), 8)
            .expect("destination"),
        [1, 2, 3, 4, 0, 0, 0, 0]
    );
}
