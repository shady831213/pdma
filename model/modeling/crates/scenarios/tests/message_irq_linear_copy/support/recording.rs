use hardware_interface_runtime::{
    AccessKind, AccessOrigin, InterruptSource, MemoryAccess, MemoryAccessError, MemoryAddress,
    MemoryAlignment, RegisterAccess, RegisterObservation, RegisterObserver,
};
use pdma_model_contract::abi::registers::{
    IrqSourceRegisters, QueueRegisters, irq_offset, queue_offset,
};
use pdma_model_host::adapter::abi::{
    AsyncDeviceAccess, PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSourceError,
    PdmaMappedRegion, PdmaMemoryProvisioning, PdmaProvisioningError, PdmaRegionRequest,
    PdmaRegisterAccessError,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryAccessRecord {
    pub kind: AccessKind,
    pub address: MemoryAddress,
    pub length: usize,
    pub alignment: MemoryAlignment,
}

#[derive(Clone, Debug)]
pub struct RecordingMemoryAccess {
    inner: AsyncDeviceAccess,
    records: Arc<Mutex<Vec<MemoryAccessRecord>>>,
}

impl RecordingMemoryAccess {
    pub fn new(inner: AsyncDeviceAccess) -> Self {
        Self {
            inner,
            records: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn records(&self) -> Vec<MemoryAccessRecord> {
        lock(&self.records).clone()
    }
    fn record(&self, record: MemoryAccessRecord) {
        lock(&self.records).push(record);
    }
}

impl MemoryAccess for RecordingMemoryAccess {
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        MemoryAccess::read(&self.inner, address, alignment, destination)?;
        self.record(MemoryAccessRecord {
            kind: AccessKind::Read,
            address,
            length: destination.len(),
            alignment,
        });
        Ok(())
    }
    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        MemoryAccess::write(&self.inner, address, alignment, source)?;
        self.record(MemoryAccessRecord {
            kind: AccessKind::Write,
            address,
            length: source.len(),
            alignment,
        });
        Ok(())
    }
}

impl RegisterAccess for RecordingMemoryAccess {
    type Error = PdmaRegisterAccessError;
    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        RegisterAccess::read(&self.inner, offset, response).await
    }
    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        RegisterAccess::write(&self.inner, offset, request).await
    }
}

impl PdmaMemoryProvisioning for RecordingMemoryAccess {
    type Error = PdmaProvisioningError;
    fn at_epoch(self, epoch: pdma_model_contract::semantic::DeviceEpoch) -> Self {
        Self {
            inner: self.inner.at_epoch(epoch),
            records: self.records,
        }
    }
    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        self.inner.allocate_region(request).await
    }
}

impl InterruptSource for RecordingMemoryAccess {
    type Notification = PdmaInterruptNotification;
    type Ready = PdmaInterruptReady;
    type Error = PdmaInterruptSourceError;
    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        self.inner.wait_ready().await
    }
    async fn claim(
        &mut self,
        ready: Self::Ready,
    ) -> Result<Option<Self::Notification>, Self::Error> {
        self.inner.claim(ready).await
    }
}

#[derive(Debug, Default)]
pub struct RecordingRegisterObserver(Mutex<Vec<RegisterObservation>>);

impl RecordingRegisterObserver {
    pub fn snapshot(&self) -> Vec<RegisterObservation> {
        lock(&self.0).clone()
    }
}

impl RegisterObserver for RecordingRegisterObserver {
    fn observe(&self, observation: RegisterObservation) {
        lock(&self.0).push(observation);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[expect(
    clippy::expect_used,
    reason = "test transcript assertions must fail the test"
)]
pub(super) fn assert_register_transcript(accesses: &[RegisterObservation]) {
    assert!(!accesses.is_empty());
    assert!(
        accesses
            .iter()
            .enumerate()
            .all(|(index, access)| access.sequence == index as u64)
    );
    assert!(
        accesses
            .iter()
            .all(|access| access.origin == AccessOrigin::Direct)
    );
    let publication_registers = [
        queue_offset(0, QueueRegisters::ENABLE.offset()).expect("queue enable offset"),
        queue_offset(0, QueueRegisters::SQ_PRODUCER.offset()).expect("submission producer offset"),
        queue_offset(0, QueueRegisters::DOORBELL.offset()).expect("doorbell offset"),
        queue_offset(0, QueueRegisters::CQ_CONSUMER.offset()).expect("completion consumer offset"),
        irq_offset(0, 0, IrqSourceRegisters::ACK.offset()).expect("completion IRQ ack offset"),
    ];
    let publication = accesses
        .iter()
        .filter(|access| {
            access.kind == AccessKind::Write && publication_registers.contains(&access.offset)
        })
        .map(|access| (access.offset, access.request_bytes.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        publication,
        publication_registers
            .into_iter()
            .zip([
                1_u32.to_le_bytes().to_vec(),
                1_u64.to_le_bytes().to_vec(),
                1_u64.to_le_bytes().to_vec(),
                1_u64.to_le_bytes().to_vec(),
                1_u32.to_le_bytes().to_vec(),
            ])
            .collect::<Vec<_>>()
    );
    println!(
        "manual-qa register transcript: accesses={} all-direct enable -> SQ producer -> doorbell -> CQ consumer -> IRQ ack; notifications use hardware output",
        accesses.len()
    );
}

#[expect(
    clippy::expect_used,
    reason = "test memory assertions must fail the test"
)]
pub(super) fn assert_memory_accesses(records: &[MemoryAccessRecord]) {
    assert_eq!(
        records
            .iter()
            .map(|record| (record.kind, record.length, record.alignment.raw()))
            .collect::<Vec<_>>(),
        [
            (AccessKind::Write, 4, 1),
            (AccessKind::Write, 64, 1),
            (AccessKind::Read, 32, 1),
            (AccessKind::Read, 4, 1),
        ]
    );
    let payload_write = records.first().unwrap_or_else(|| std::process::abort());
    let descriptor_write = records
        .get(1)
        .expect("host wrote the submission descriptor");
    let completion_read = records.get(2).expect("host read the completion");
    let payload_read = records.last().unwrap_or_else(|| std::process::abort());
    println!(
        "manual-qa bulk memory: records={} payload-write={:#x}/4/1 descriptor-write={:#x}/64/1 completion-read={:#x}/32/1 payload-read={:#x}/4/1",
        records.len(),
        payload_write.address.raw(),
        descriptor_write.address.raw(),
        completion_read.address.raw(),
        payload_read.address.raw()
    );
}
