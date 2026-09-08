use super::MmioRegisterError;
use super::preparation::{prepare_read, prepare_write};
use crate::runtime::{DeviceCore, PdmaMemoryProvisioning, RunnerShared};
use crate::runtime::{backend::DeviceInteractionBackend, state::RunnerState};
use hardware_interface_runtime::{
    AccessOrigin, NoopRegisterObserver, ObservationGate, RegisterAccess, RegisterBusAddress,
    RegisterBusBase, RegisterBusOffset, RegisterBusWidth, SharedRegisterObserver,
};
use pdma_model_contract::abi::registers::{decode_read, decode_write};
use pdma_model_contract::semantic::DeviceEpoch;
use std::sync::Arc;

/// Cloneable exact register endpoint over one device core.
#[derive(Clone, Debug)]
pub struct MmioRegisterEndpoint {
    pub(crate) shared: Arc<RunnerShared>,
    mmio_base: RegisterBusBase,
    epoch: DeviceEpoch,
}

impl MmioRegisterEndpoint {
    /// Consumes a core into a standalone exact register endpoint.
    pub fn from_core(core: DeviceCore) -> Self {
        let epoch = core.epoch();
        let provisioning = PdmaMemoryProvisioning::external(core.memory.memory_access());
        Self::new(
            Arc::new(RunnerShared {
                backend: DeviceInteractionBackend::new(RunnerState::new(core, provisioning)),
                register_gate: ObservationGate::new(SharedRegisterObserver::new(Arc::new(
                    NoopRegisterObserver,
                ))),
            }),
            epoch,
        )
    }

    /// Consumes a core into an exact register endpoint at `mmio_base`.
    pub fn from_core_at_mmio_base(core: DeviceCore, mmio_base: RegisterBusBase) -> Self {
        Self::from_core(core).at_mmio_base(mmio_base)
    }

    pub(crate) const fn new(shared: Arc<RunnerShared>, epoch: DeviceEpoch) -> Self {
        Self {
            shared,
            mmio_base: RegisterBusBase::new(0),
            epoch,
        }
    }

    /// Returns a clone over the same core rooted at `mmio_base`.
    #[must_use]
    pub fn at_mmio_base(&self, mmio_base: RegisterBusBase) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            mmio_base,
            epoch: self.epoch,
        }
    }

    /// Binds a clone to an explicitly discovered epoch without changing other clones.
    #[must_use]
    pub fn at_epoch(&self, epoch: DeviceEpoch) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            mmio_base: self.mmio_base,
            epoch,
        }
    }

    /// Returns the epoch required for writes; non-consuming reads permit rediscovery.
    pub const fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }

    /// Returns this endpoint's configured full-address base.
    pub const fn mmio_base(&self) -> RegisterBusBase {
        self.mmio_base
    }

    fn full_address(&self, offset: u32) -> Result<RegisterBusAddress, MmioRegisterError> {
        self.mmio_base
            .raw()
            .checked_add(u64::from(offset))
            .map(RegisterBusAddress::new)
            .ok_or(MmioRegisterError::Access(
                pdma_model_contract::abi::registers::MmioAccessError::UnmappedOffset { offset },
            ))
    }

    fn canonical_offset(
        &self,
        address: RegisterBusAddress,
        width: usize,
        fallback: u32,
    ) -> Result<u32, MmioRegisterError> {
        RegisterBusOffset::from_address(self.mmio_base, address, RegisterBusWidth::new(width))
            .map(RegisterBusOffset::raw)
            .map_err(|_| {
                MmioRegisterError::Access(
                    pdma_model_contract::abi::registers::MmioAccessError::UnmappedOffset {
                        offset: fallback,
                    },
                )
            })
    }
}

impl RegisterAccess for MmioRegisterEndpoint {
    type Error = MmioRegisterError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        let address = self.full_address(offset)?;
        let offset = self.canonical_offset(address, response.len(), offset)?;
        let register = decode_read(offset, response.len() as u128)?;
        let (prepared, reservation) = self
            .shared
            .backend
            .interact(|state| -> Result<_, MmioRegisterError> {
                let prepared = prepare_read(&state.core, register, offset)?;
                let reservation = self.shared.register_gate.reserve(1)?;
                Ok((prepared, reservation))
            })
            .await?;
        let observed = prepared.observation(reservation.start_sequence());
        response.copy_from_slice(&prepared.response);
        reservation.emit(vec![observed])?;
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        let address = self.full_address(offset)?;
        let offset = self.canonical_offset(address, request.len(), offset)?;
        let (register, value) = decode_write(offset, request)?;
        self.shared
            .backend
            .commit(
                |state| -> Result<_, MmioRegisterError> {
                    let expected = state.core.epoch();
                    if self.epoch != expected {
                        return Err(MmioRegisterError::StaleEpoch {
                            expected,
                            actual: self.epoch,
                        });
                    }
                    let prepared = prepare_write(register, value, offset, request);
                    let reservation = self.shared.register_gate.reserve(1)?;
                    let changed = match prepared.commit(&mut state.core) {
                        Ok(changed) => changed,
                        Err(error) => {
                            // This per-core gate can only reserve while the same backend lock is held.
                            reservation.cancel_latest()?;
                            return Err(error.into());
                        }
                    };
                    Ok(((prepared, reservation), changed))
                },
                |(prepared, reservation)| {
                    let observed =
                        prepared.observation(reservation.start_sequence(), AccessOrigin::Direct);
                    reservation
                        .emit(vec![observed])
                        .map_err(MmioRegisterError::CommittedObservation)?;
                    Ok(())
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests;
