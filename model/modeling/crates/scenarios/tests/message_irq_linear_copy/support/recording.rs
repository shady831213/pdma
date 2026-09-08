use hardware_interface_runtime::{
    AccessKind, AccessOrigin, MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment,
    RegisterAccess, RegisterObservation, RegisterObserver,
};
use pdma_model_host::adapter::abi::{
    AsyncDeviceAccess, PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim, PdmaMappedRegion,
    PdmaMemoryProvisioning, PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError,
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
    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        self.inner.allocate_region(request).await
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
    let publication = accesses
        .iter()
        .filter(|access| {
            access.kind == AccessKind::Write
                && access.offset == 0x168
                && matches!(access.request_bytes.as_slice(), [1 | 3 | 4, 0, 0, 0])
        })
        .map(|access| access.request_bytes.as_slice())
        .collect::<Vec<_>>();
    assert_eq!(
        publication,
        [&[1, 0, 0, 0][..], &[3, 0, 0, 0], &[4, 0, 0, 0]]
    );
    let claim_start = accesses.iter().position(|access| {
        access.origin == AccessOrigin::Direct
            && access.kind == AccessKind::Write
            && access.offset == 0x220
            && access.request_bytes == [9, 0, 0, 0]
    });
    let claim_start = claim_start.expect("one exact claim must occur");
    let sequence = u64::try_from(claim_start).expect("test sequence fits");
    let exact = [
        RegisterObservation::write(sequence, AccessOrigin::Direct, 0x220, &[9, 0, 0, 0]),
        RegisterObservation::read(sequence + 1, AccessOrigin::Direct, 0x224, &[1, 0, 0, 0]),
        RegisterObservation::read(sequence + 2, AccessOrigin::Direct, 0x240, &[0; 8]),
        RegisterObservation::read(sequence + 3, AccessOrigin::Direct, 0x248, &[0; 8]),
        RegisterObservation::read(sequence + 4, AccessOrigin::Direct, 0x250, &[0; 4]),
        RegisterObservation::read(sequence + 5, AccessOrigin::Direct, 0x254, &[75, 0, 0, 0]),
        RegisterObservation::read(sequence + 6, AccessOrigin::Direct, 0x258, &[1, 0, 0, 0]),
        RegisterObservation::read(sequence + 7, AccessOrigin::Direct, 0x25c, &[2, 0, 0, 0]),
        RegisterObservation::read(
            sequence + 8,
            AccessOrigin::Direct,
            0x260,
            &[64, 0, 0, 0, 0, 0, 0, 0],
        ),
        RegisterObservation::read(sequence + 9, AccessOrigin::Direct, 0x268, &[0; 8]),
        RegisterObservation::read(sequence + 10, AccessOrigin::Direct, 0x270, &[0; 8]),
    ];
    assert_eq!(
        accesses.get(claim_start..claim_start + exact.len()),
        Some(exact.as_slice())
    );
    println!(
        "manual-qa register transcript: accesses={} all-direct publication=[1,3,4] claim-index={claim_start} claim-items=11",
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
