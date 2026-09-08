mod mapping;
#[cfg(test)]
mod tests;

use self::mapping::{
    map_model_allocation, map_register, project_region, read_lock, validate_alignment, write_lock,
};
use crate::adapter::abi::{
    PdmaInterruptNotification, PdmaInterruptReady, PdmaInterruptSourceError, PdmaMappedRegion,
    PdmaMemoryProvisioning, PdmaProvisioningError, PdmaRegionRequest, PdmaRegisterAccessError,
};
use hardware_interface_runtime::{
    AddressedRegisterAccess, InterruptSource, MemoryAccess, MemoryAccessError, MemoryAddress,
    MemoryAlignment, RegisterAccess, RegisterBusAddress, RegisterBusBase, RegisterBusError,
    SharedMemoryAccess,
};
use pdma_model_contract::semantic::DeviceEpoch;
use pdma_model_device::AsyncDeviceHandle;
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

/// One-core concrete canonical access adapter.
#[derive(Clone, Debug)]
pub struct AsyncDeviceAccess {
    handle: AsyncDeviceHandle,
    memory: SharedMemoryAccess,
    mmio_base: RegisterBusBase,
    zero_bases: Arc<RwLock<BTreeSet<MemoryAddress>>>,
    epoch: DeviceEpoch,
}

impl AsyncDeviceAccess {
    /// Derives every capability from one existing asynchronous device handle.
    pub fn new(handle: AsyncDeviceHandle) -> Self {
        Self::new_at_mmio_base(handle, RegisterBusBase::new(0))
    }

    /// Derives every capability with register traffic rooted at `mmio_base`.
    pub fn new_at_mmio_base(handle: AsyncDeviceHandle, mmio_base: RegisterBusBase) -> Self {
        Self {
            memory: handle.memory_access(),
            handle,
            mmio_base,
            zero_bases: Arc::new(RwLock::new(BTreeSet::new())),
            epoch: DeviceEpoch::initial(),
        }
    }

    /// Binds subsequent hardware accesses to the supplied reset epoch.
    #[must_use]
    pub const fn at_epoch(mut self, epoch: DeviceEpoch) -> Self {
        self.epoch = epoch;
        self
    }

    /// Returns the full-address base used for register traffic.
    pub const fn mmio_base(&self) -> RegisterBusBase {
        self.mmio_base
    }

    fn register_address(&self, offset: u32) -> Result<RegisterBusAddress, PdmaRegisterAccessError> {
        self.mmio_base
            .raw()
            .checked_add(u64::from(offset))
            .map(RegisterBusAddress::new)
            .ok_or(PdmaRegisterAccessError::Access(
                pdma_model_contract::abi::registers::MmioAccessError::UnmappedOffset { offset },
            ))
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
        let address = self.register_address(offset)?;
        let endpoint = self
            .handle
            .register_endpoint()
            .at_mmio_base(self.mmio_base)
            .at_epoch(self.epoch);
        endpoint
            .read_addressed(self.mmio_base, address, response)
            .await
            .map_err(|error| map_addressed_register(error, offset))
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        let address = self.register_address(offset)?;
        let endpoint = self
            .handle
            .register_endpoint()
            .at_mmio_base(self.mmio_base)
            .at_epoch(self.epoch);
        endpoint
            .write_addressed(self.mmio_base, address, request)
            .await
            .map_err(|error| map_addressed_register(error, offset))
    }
}

fn map_addressed_register(
    error: RegisterBusError<pdma_model_device::MmioRegisterError>,
    fallback: u32,
) -> PdmaRegisterAccessError {
    match error {
        RegisterBusError::Endpoint { source, .. } => map_register(source),
        RegisterBusError::AddressBelowBase { .. } | RegisterBusError::OffsetOutOfRange { .. } => {
            PdmaRegisterAccessError::Access(
                pdma_model_contract::abi::registers::MmioAccessError::UnmappedOffset {
                    offset: fallback,
                },
            )
        }
    }
}

impl PdmaMemoryProvisioning for AsyncDeviceAccess {
    type Error = PdmaProvisioningError;

    fn at_epoch(self, epoch: DeviceEpoch) -> Self {
        Self::at_epoch(self, epoch)
    }

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
}

impl InterruptSource for AsyncDeviceAccess {
    type Notification = PdmaInterruptNotification;
    type Ready = PdmaInterruptReady;
    type Error = PdmaInterruptSourceError;

    async fn wait_ready(&self) -> Result<Self::Ready, Self::Error> {
        self.handle
            .wait_irq_notification_ready_at_epoch(self.epoch)
            .await
            .map_err(mapping::map_interrupt)?
            .then_some(PdmaInterruptReady::new(self.epoch))
            .ok_or(PdmaInterruptSourceError::Closed)
    }

    async fn claim(
        &mut self,
        ready: Self::Ready,
    ) -> Result<Option<Self::Notification>, Self::Error> {
        if ready.epoch() != self.epoch {
            return Err(PdmaInterruptSourceError::StaleEpoch {
                expected: self.epoch,
                actual: ready.epoch(),
            });
        }
        self.handle
            .take_irq_notification_at_epoch(self.epoch)
            .await
            .map(|notification| notification.map(PdmaInterruptNotification::new))
            .map_err(mapping::map_interrupt)
    }
}
