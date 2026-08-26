mod mapping;

use self::mapping::{
    device_write, map_batch, map_claim, map_memory, map_model_allocation, map_register,
    project_region, read_lock, validate_alignment, write_lock,
};
use crate::register::{
    PdmaBatchAccess, PdmaBatchError, PdmaClaimError, PdmaClaimResult, PdmaExactInterruptClaim,
    PdmaMappedRegion, PdmaMemoryProvisioning, PdmaPreparedProposal, PdmaProvisioningError,
    PdmaRegionRequest, PdmaRegisterAccessError, PdmaSubmissionBatch,
};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
};
use pdma_model_contract::binding::QueueActivation;
use pdma_model_device::AsyncDeviceHandle;
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

/// One-core concrete canonical access adapter.
#[derive(Clone, Debug)]
pub struct AsyncDeviceAccess {
    handle: AsyncDeviceHandle,
    memory: Arc<SharedMemory>,
    zero_bases: Arc<RwLock<BTreeSet<MemoryAddress>>>,
}

impl AsyncDeviceAccess {
    /// Derives every capability from one existing asynchronous device handle.
    pub fn new(handle: AsyncDeviceHandle) -> Self {
        Self {
            memory: handle.memory_access(),
            handle,
            zero_bases: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }

    fn remember_zero_regions(&self, regions: &[PdmaMappedRegion]) {
        let mut zero_bases = write_lock(&self.zero_bases);
        zero_bases.extend(
            regions
                .iter()
                .filter(|region| region.length() == 0)
                .map(|region| region.base()),
        );
    }

    fn is_logical_zero_access(&self, address: MemoryAddress, length: usize) -> bool {
        length == 0 && read_lock(&self.zero_bases).contains(&address)
    }
}

impl MemoryAccess for AsyncDeviceAccess {
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        if self.is_logical_zero_access(address, destination.len()) {
            return Ok(());
        }
        self.memory.read(address, alignment, destination)
    }

    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), MemoryAccessError> {
        if self.is_logical_zero_access(address, source.len()) {
            return Ok(());
        }
        self.memory.write(address, alignment, source)
    }
}

impl RegisterAccess for AsyncDeviceAccess {
    type Error = PdmaRegisterAccessError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        RegisterAccess::read(&self.handle.register_endpoint(), offset, response)
            .await
            .map_err(map_register)
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        RegisterAccess::write(&self.handle.register_endpoint(), offset, request)
            .await
            .map_err(map_register)
    }
}

/// Concrete opaque prepared proposal; the device stage is never exposed.
#[derive(Debug)]
pub struct AsyncDevicePreparedRegions {
    stage: pdma_model_device::PdmaMemoryStage,
    regions: Vec<PdmaMappedRegion>,
}

impl PdmaPreparedProposal for AsyncDevicePreparedRegions {
    fn regions(&self) -> &[PdmaMappedRegion] {
        &self.regions
    }
    fn generation(&self) -> u64 {
        self.stage.generation()
    }
}

impl PdmaMemoryProvisioning for AsyncDeviceAccess {
    type Proposal = AsyncDevicePreparedRegions;
    type Error = PdmaProvisioningError;

    async fn allocate_region(
        &self,
        request: PdmaRegionRequest,
    ) -> Result<PdmaMappedRegion, Self::Error> {
        validate_alignment(request.alignment())?;
        let region = self
            .handle
            .allocate_memory_region(
                usize::try_from(request.length())
                    .map_err(|_| PdmaProvisioningError::LengthOutOfRange)?,
            )
            .await
            .map_err(map_model_allocation)?;
        let mapped = project_region(region, request.alignment())?;
        self.remember_zero_regions(&[mapped]);
        Ok(mapped)
    }

    async fn prepare_regions(
        &self,
        requests: &[PdmaRegionRequest],
    ) -> Result<Self::Proposal, Self::Error> {
        for request in requests {
            validate_alignment(request.alignment())?;
        }
        let lengths = requests
            .iter()
            .map(|request| request.length())
            .collect::<Vec<_>>();
        let stage = self
            .handle
            .stage_memory_regions(&lengths)
            .await
            .map_err(map_memory)?;
        let regions = stage
            .regions()
            .iter()
            .zip(requests)
            .map(|(region, request)| project_region(*region, request.alignment()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AsyncDevicePreparedRegions { stage, regions })
    }
}

impl PdmaBatchAccess for AsyncDeviceAccess {
    type BatchError = PdmaBatchError;

    async fn bootstrap_queue(
        &self,
        proposal: Self::Proposal,
        activation: QueueActivation,
    ) -> Result<[u8; 72], Self::BatchError> {
        let regions = proposal.regions.clone();
        let request = pdma_model_device::QueueBootstrapBatch::new(proposal.stage, activation);
        let response = self
            .handle
            .bootstrap_queue_batch(request)
            .await
            .map_err(map_batch)?;
        self.remember_zero_regions(&regions);
        Ok(response)
    }

    async fn submit_queue(
        &self,
        request: PdmaSubmissionBatch<Self::Proposal>,
    ) -> Result<[u8; 72], Self::BatchError> {
        let (queue, producer, descriptor, indirect) = request.into_parts();
        let descriptor = device_write(descriptor);
        let (indirect, regions) = match indirect {
            Some((proposal, write)) => {
                let regions = proposal.regions.clone();
                (Some((proposal.stage, device_write(write))), regions)
            }
            None => (None, Vec::new()),
        };
        let request =
            pdma_model_device::SubmissionBatch::new(queue, producer, descriptor, indirect);
        let response = self
            .handle
            .submit_queue_batch(request)
            .await
            .map_err(map_batch)?;
        self.remember_zero_regions(&regions);
        Ok(response)
    }
}

impl PdmaExactInterruptClaim for AsyncDeviceAccess {
    type Error = PdmaClaimError;

    async fn wait_interrupt_ready(&self) -> Result<bool, Self::Error> {
        Ok(self.handle.wait_irq_notification_ready().await)
    }

    async fn claim_interrupt_exact(&self) -> Result<PdmaClaimResult, Self::Error> {
        if !self.handle.wait_irq_notification_ready().await {
            return Ok(PdmaClaimResult::Shutdown);
        }
        self.handle
            .register_endpoint()
            .claim_notification_exact_observed()
            .await
            .map(|(notification, observations)| {
                notification.map_or(PdmaClaimResult::Empty, |notification| {
                    PdmaClaimResult::Notification {
                        notification,
                        observations,
                    }
                })
            })
            .map_err(map_claim)
    }
}
