use super::super::bus::{BusDmaStage, BusDmaWrite, BusSubmission, MmioDeviceTransport};
use super::super::{
    PdmaBatchError, PdmaDmaWrite, PdmaPreparedProposal, PdmaProvisioningCause,
    PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError, PdmaSubmissionBatch,
};
use crate::{ModelError, ModelResult};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess,
};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount};
use pdma_model_contract::bridge::{PdmaMemoryRegion, QueueActivation};
use pdma_model_device::{MmioBatchError, PdmaMemoryError};

#[derive(Debug, Clone)]
pub(super) struct CanonicalTransport<A> {
    access: A,
}

impl<A> CanonicalTransport<A> {
    pub(super) const fn new(access: A) -> Self {
        Self { access }
    }
}

#[derive(Debug)]
pub(super) struct PortStage<P> {
    proposal: P,
    regions: Vec<PdmaMemoryRegion>,
}

impl<P: PdmaPreparedProposal> PortStage<P> {
    fn project(proposal: P) -> Result<Self, PdmaMemoryError> {
        let regions = proposal
            .regions()
            .iter()
            .copied()
            .map(project_region)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { proposal, regions })
    }
}

impl<P: PdmaPreparedProposal> BusDmaStage for PortStage<P> {
    fn proposed_regions(&self) -> &[PdmaMemoryRegion] {
        &self.regions
    }
}

impl<A> MmioDeviceTransport for CanonicalTransport<A>
where
    A: super::CanonicalAccess,
    A::Proposal: Send,
{
    type Stage = PortStage<A::Proposal>;

    async fn allocate_memory_region(&self, len: usize) -> ModelResult<PdmaMemoryRegion> {
        let length = u64::try_from(len).map_err(|_| ModelError::DmaAllocationExhausted)?;
        let alignment =
            MemoryAlignment::try_new(64).map_err(|_| ModelError::DmaAllocationExhausted)?;
        self.access
            .allocate_region(PdmaRegionRequest::new(length, alignment))
            .await
            .map_err(|_| ModelError::DmaAllocationExhausted)
            .and_then(|region| {
                project_region(region).map_err(|_| ModelError::DmaAllocationExhausted)
            })
    }

    async fn stage_memory_regions(&self, lengths: &[u64]) -> Result<Self::Stage, PdmaMemoryError> {
        let alignment = MemoryAlignment::try_new(64).map_err(|_| PdmaMemoryError::InvalidRegion)?;
        let requests = lengths
            .iter()
            .copied()
            .map(|length| PdmaRegionRequest::new(length, alignment))
            .collect::<Vec<_>>();
        let proposal = self
            .access
            .prepare_regions(&requests)
            .await
            .map_err(map_provisioning)?;
        PortStage::project(proposal)
    }

    async fn read_dma(&self, address: DmaAddress, length: u64) -> Result<Vec<u8>, PdmaMemoryError> {
        let len = usize::try_from(length).map_err(|_| PdmaMemoryError::AddressExhausted)?;
        let mut bytes = vec![0; len];
        MemoryAccess::read(
            &self.access,
            MemoryAddress::new(address.raw()),
            MemoryAlignment::ONE,
            &mut bytes,
        )
        .map_err(map_memory)?;
        Ok(bytes)
    }

    async fn write_dma(&self, address: DmaAddress, bytes: &[u8]) -> Result<(), PdmaMemoryError> {
        MemoryAccess::write(
            &self.access,
            MemoryAddress::new(address.raw()),
            MemoryAlignment::ONE,
            bytes,
        )
        .map_err(map_memory)
    }

    async fn bootstrap_batch(
        &self,
        stage: Self::Stage,
        activation: QueueActivation,
    ) -> Result<[u8; 72], MmioBatchError> {
        self.access
            .bootstrap_queue(stage.proposal, activation)
            .await
            .map_err(map_batch)
    }

    async fn submission_batch(
        &self,
        request: BusSubmission<Self::Stage>,
    ) -> Result<[u8; 72], MmioBatchError> {
        let (queue, producer, descriptor, indirect) = request.into_parts();
        let descriptor = map_write(descriptor);
        let indirect = indirect.map(|(stage, write)| (stage.proposal, map_write(write)));
        self.access
            .submit_queue(PdmaSubmissionBatch::new(
                queue, producer, descriptor, indirect,
            ))
            .await
            .map_err(map_batch)
    }

    async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        let mut bytes = vec![
            0;
            usize::try_from(width)
                .map_err(|_| MmioAccessError::ReservedOffset { offset })?
        ];
        RegisterAccess::read(&self.access, offset, &mut bytes)
            .await
            .map_err(map_register)?;
        Ok(bytes)
    }

    async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        RegisterAccess::write(&self.access, offset, bytes)
            .await
            .map_err(map_register)
    }

    async fn wait_irq_notification_ready(&self) -> bool {
        true
    }
}

fn map_write(write: BusDmaWrite) -> PdmaDmaWrite {
    let (address, bytes) = write.into_parts();
    PdmaDmaWrite::new(MemoryAddress::new(address.raw()), bytes)
}

fn project_region(
    region: super::super::PdmaMappedRegion,
) -> Result<PdmaMemoryRegion, PdmaMemoryError> {
    PdmaMemoryRegion::try_new(
        region.object(),
        DmaAddress::new(region.base().raw()),
        region.length(),
    )
    .map_err(|_| PdmaMemoryError::InvalidRegion)
}

const fn map_register(error: PdmaRegisterAccessError) -> MmioAccessError {
    match error {
        PdmaRegisterAccessError::Access(error) => error,
        PdmaRegisterAccessError::Observation(_) => {
            MmioAccessError::ReservedOffset { offset: u32::MAX }
        }
    }
}

const fn map_memory(error: MemoryAccessError) -> PdmaMemoryError {
    match error {
        MemoryAccessError::AddressOverflow { .. } => PdmaMemoryError::AddressExhausted,
        MemoryAccessError::Unmapped { .. } => PdmaMemoryError::Unmapped,
        MemoryAccessError::CrossesRegion { .. } => PdmaMemoryError::CrossesRegion,
        MemoryAccessError::Misaligned { .. } => PdmaMemoryError::InvalidRegion,
    }
}

const fn map_provisioning(error: PdmaProvisioningError) -> PdmaMemoryError {
    match error {
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::AddressExhausted)
        | PdmaProvisioningError::LengthOutOfRange => PdmaMemoryError::AddressExhausted,
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::StorageExhausted) => {
            PdmaMemoryError::StorageExhausted
        }
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::Unmapped) => {
            PdmaMemoryError::Unmapped
        }
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::CrossesRegion) => {
            PdmaMemoryError::CrossesRegion
        }
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::OverlappingRegion) => {
            PdmaMemoryError::OverlappingRegion
        }
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::StaleProposal {
            expected,
            actual,
        }) => PdmaMemoryError::StaleStage {
            expected_generation: expected,
            actual_generation: actual,
        },
        PdmaProvisioningError::Provisioning(PdmaProvisioningCause::InvalidRegion)
        | PdmaProvisioningError::Access(_)
        | PdmaProvisioningError::InvalidProjection => PdmaMemoryError::InvalidRegion,
    }
}

const fn map_batch(error: PdmaBatchError) -> MmioBatchError {
    match error {
        PdmaBatchError::Access(error) => MmioBatchError::Access(error),
        PdmaBatchError::Result(error) => MmioBatchError::Command(error),
        PdmaBatchError::Observation(error) => MmioBatchError::Observation(error),
        PdmaBatchError::Provisioning(_) | PdmaBatchError::InvalidShape => {
            MmioBatchError::InvalidShape
        }
    }
}
