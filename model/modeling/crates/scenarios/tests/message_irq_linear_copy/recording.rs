use hardware_interface_runtime::{
    AccessKind, AccessOrigin, MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment,
    RegisterAccess, RegisterObservation, RegisterObserver,
};
use pdma_model_contract::bridge::QueueActivation;
use pdma_model_host::{
    adapters::{AsyncDeviceAccess, AsyncDevicePreparedRegions},
    register::{
        PdmaBatchAccess, PdmaBatchError, PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim,
        PdmaMappedRegion, PdmaMemoryProvisioning, PdmaProvisioningError, PdmaRegionRequest,
        PdmaRegisterAccessError, PdmaSubmissionBatch,
    },
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
    type Proposal = AsyncDevicePreparedRegions;
    type Error = PdmaProvisioningError;
    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        self.inner.allocate_region(request).await
    }
    async fn prepare_regions(
        &self,
        requests: &[PdmaRegionRequest],
    ) -> Result<Self::Proposal, Self::Error> {
        self.inner.prepare_regions(requests).await
    }
}

impl PdmaBatchAccess for RecordingMemoryAccess {
    type BatchError = PdmaBatchError;
    async fn bootstrap_queue(
        &self,
        proposal: Self::Proposal,
        activation: QueueActivation,
    ) -> Result<[u8; 72], Self::BatchError> {
        self.inner.bootstrap_queue(proposal, activation).await
    }
    async fn submit_queue(
        &self,
        request: PdmaSubmissionBatch<Self::Proposal>,
    ) -> Result<[u8; 72], Self::BatchError> {
        self.inner.submit_queue(request).await
    }
}

impl PdmaExactInterruptClaim for RecordingMemoryAccess {
    type Error = PdmaClaimError;
    async fn wait_interrupt_ready(&self) -> Result<bool, Self::Error> {
        self.inner.wait_interrupt_ready().await
    }
    async fn claim_interrupt_exact(&self) -> Result<PdmaClaimResult, Self::Error> {
        self.inner.claim_interrupt_exact().await
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

pub(super) fn assert_register_transcript(accesses: &[RegisterObservation]) {
    assert_eq!(accesses.len(), 156);
    assert!(
        accesses
            .iter()
            .enumerate()
            .all(|(index, access)| access.sequence == index as u64)
    );
    let batch = accesses
        .iter()
        .filter(|access| access.origin == AccessOrigin::Batch)
        .collect::<Vec<_>>();
    assert_eq!(batch.len(), 2);
    let bootstrap = batch.first().unwrap_or_else(|| std::process::abort());
    let submission = batch.last().unwrap_or_else(|| std::process::abort());
    assert_eq!(
        (
            bootstrap.kind,
            bootstrap.offset,
            bootstrap.request_bytes.as_slice()
        ),
        (AccessKind::Write, 0x168, &[1, 0, 0, 0][..])
    );
    assert_eq!(
        (
            submission.kind,
            submission.offset,
            submission.request_bytes.as_slice()
        ),
        (AccessKind::Write, 0x168, &[3, 0, 0, 0][..])
    );
    assert_eq!(
        accesses
            .iter()
            .filter(|access| access.origin == AccessOrigin::Direct)
            .count(),
        154
    );
    let claim_start = accesses.iter().position(|access| {
        access.origin == AccessOrigin::Direct
            && access.kind == AccessKind::Write
            && access.offset == 0x220
            && access.request_bytes == [9, 0, 0, 0]
    });
    assert_eq!(claim_start, Some(74));
    let exact = [
        RegisterObservation::write(74, AccessOrigin::Direct, 0x220, &[9, 0, 0, 0]),
        RegisterObservation::read(75, AccessOrigin::Direct, 0x224, &[1, 0, 0, 0]),
        RegisterObservation::read(76, AccessOrigin::Direct, 0x240, &[0; 8]),
        RegisterObservation::read(77, AccessOrigin::Direct, 0x248, &[0; 8]),
        RegisterObservation::read(78, AccessOrigin::Direct, 0x250, &[0; 4]),
        RegisterObservation::read(79, AccessOrigin::Direct, 0x254, &[75, 0, 0, 0]),
        RegisterObservation::read(80, AccessOrigin::Direct, 0x258, &[1, 0, 0, 0]),
        RegisterObservation::read(81, AccessOrigin::Direct, 0x25c, &[2, 0, 0, 0]),
        RegisterObservation::read(82, AccessOrigin::Direct, 0x260, &[64, 0, 0, 0, 0, 0, 0, 0]),
        RegisterObservation::read(83, AccessOrigin::Direct, 0x268, &[0; 8]),
        RegisterObservation::read(84, AccessOrigin::Direct, 0x270, &[0; 8]),
    ];
    assert_eq!(accesses.get(74..85), Some(exact.as_slice()));
    println!(
        "manual-qa register transcript: accesses={} direct=154 batch=2 batch-values=[1,3] claim-index=74 claim-items=11",
        accesses.len()
    );
}

pub(super) fn assert_memory_accesses(records: &[MemoryAccessRecord]) {
    assert_eq!(
        records
            .iter()
            .map(|record| (record.kind, record.length, record.alignment.raw()))
            .collect::<Vec<_>>(),
        [
            (AccessKind::Write, 4, 1),
            (AccessKind::Read, 32, 1),
            (AccessKind::Read, 4, 1),
        ]
    );
    let payload_write = records.first().unwrap_or_else(|| std::process::abort());
    let descriptor_read = records.get(1).unwrap_or_else(|| std::process::abort());
    let payload_read = records.last().unwrap_or_else(|| std::process::abort());
    println!(
        "manual-qa bulk memory: records={} payload-write={:#x}/4/1 descriptor-read={:#x}/32/1 payload-read={:#x}/4/1",
        records.len(),
        payload_write.address.raw(),
        descriptor_read.address.raw(),
        payload_read.address.raw()
    );
}
