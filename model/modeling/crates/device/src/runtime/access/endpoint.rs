use crate::interface::mmio::{
    MmioRegisterError, claim_observation_count, prepare_claim, prepare_read, prepare_write,
};
use crate::runtime::{AsyncDevice, DeviceCore, RunnerShared};
use hardware_interface_runtime::{
    AccessOrigin, RegisterAccess, RegisterBusAddress, RegisterBusBase, RegisterBusOffset,
    RegisterBusWidth, RegisterObservation,
};
use pdma_model_contract::abi::mmio::{decode_read, decode_write};
use std::sync::Arc;

/// Cloneable exact register endpoint over one device core.
#[derive(Clone, Debug)]
pub struct MmioRegisterEndpoint {
    pub(crate) shared: Arc<RunnerShared>,
    mmio_base: RegisterBusBase,
}

impl MmioRegisterEndpoint {
    /// Consumes a core into a standalone exact register endpoint.
    pub fn from_core(core: DeviceCore) -> Self {
        let (device, _worker) = AsyncDevice::from_core(core);
        device.handle().register_endpoint()
    }

    /// Consumes a core into an exact register endpoint at `mmio_base`.
    pub fn from_core_at_mmio_base(core: DeviceCore, mmio_base: RegisterBusBase) -> Self {
        Self::from_core(core).at_mmio_base(mmio_base)
    }

    pub(crate) const fn new(shared: Arc<RunnerShared>) -> Self {
        Self {
            shared,
            mmio_base: RegisterBusBase::new(0),
        }
    }

    /// Returns a clone over the same core rooted at `mmio_base`.
    #[must_use]
    pub fn at_mmio_base(&self, mmio_base: RegisterBusBase) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            mmio_base,
        }
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
                pdma_model_contract::abi::mmio::MmioAccessError::UnmappedOffset { offset },
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
                    pdma_model_contract::abi::mmio::MmioAccessError::UnmappedOffset {
                        offset: fallback,
                    },
                )
            })
    }

    /// Claims one exact IRQ notification and emits its complete direct transcript atomically.
    pub async fn claim_notification_exact(
        &self,
    ) -> Result<Option<pdma_model_contract::semantic::IrqNotification>, MmioRegisterError> {
        self.shared
            .backend
            .commit(
                |state| -> Result<_, MmioRegisterError> {
                    let reservation = self
                        .shared
                        .register_gate
                        .reserve(claim_observation_count(&state.core))?;
                    let committed = prepare_claim(&mut state.core, reservation.start_sequence())?;
                    let changed = committed.changed();
                    Ok(((committed, reservation), changed))
                },
                |(committed, reservation)| {
                    let (notification, observations) = committed.into_parts();
                    reservation.emit(observations)?;
                    Ok(notification)
                },
            )
            .await
    }

    /// Claims one IRQ notification and returns the exact emitted direct transcript.
    pub async fn claim_notification_exact_observed(
        &self,
    ) -> Result<
        (
            Option<pdma_model_contract::semantic::IrqNotification>,
            Vec<RegisterObservation>,
        ),
        MmioRegisterError,
    > {
        self.shared
            .backend
            .commit(
                |state| -> Result<_, MmioRegisterError> {
                    let reservation = self
                        .shared
                        .register_gate
                        .reserve(claim_observation_count(&state.core))?;
                    let committed = prepare_claim(&mut state.core, reservation.start_sequence())?;
                    let changed = committed.changed();
                    Ok(((committed, reservation), changed))
                },
                |(committed, reservation)| {
                    let (notification, observations) = committed.into_parts();
                    reservation.emit(observations.clone())?;
                    Ok((notification, observations))
                },
            )
            .await
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
                let prepared = prepare_read(&state.core, register, offset);
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
                    let prepared = prepare_write(register, value, offset, request);
                    let reservation = self.shared.register_gate.reserve(1)?;
                    let changed = prepared.commit(&mut state.core);
                    Ok(((prepared, reservation), changed))
                },
                |(prepared, reservation)| {
                    let observed =
                        prepared.observation(reservation.start_sequence(), AccessOrigin::Direct);
                    reservation.emit(vec![observed])?;
                    Ok(())
                },
            )
            .await
    }
}
