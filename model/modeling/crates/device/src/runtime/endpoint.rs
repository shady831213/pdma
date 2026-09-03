use super::{AsyncDevice, DeviceCore, RunnerShared, wake};
use crate::interface::mmio::{
    MmioRegisterError, claim_observation_count, prepare_claim, prepare_read, prepare_write,
};
use hardware_interface_runtime::{AccessOrigin, RegisterAccess, RegisterObservation};
use pdma_model_contract::abi::mmio::{decode_read, decode_write};
use std::sync::Arc;

/// Cloneable exact register endpoint over one device core.
#[derive(Clone, Debug)]
pub struct MmioRegisterEndpoint {
    pub(crate) shared: Arc<RunnerShared>,
}

impl MmioRegisterEndpoint {
    /// Consumes a core into a standalone exact register endpoint.
    pub fn from_core(core: DeviceCore) -> Self {
        let (device, _worker) = AsyncDevice::from_core(core);
        device.handle().register_endpoint()
    }

    pub(crate) const fn new(shared: Arc<RunnerShared>) -> Self {
        Self { shared }
    }

    /// Claims one exact IRQ notification and emits its complete direct transcript atomically.
    pub async fn claim_notification_exact(
        &self,
    ) -> Result<Option<pdma_model_contract::semantic::IrqNotification>, MmioRegisterError> {
        let (committed, reservation, generation) = self
            .shared
            .backend
            .interact(|state| -> Result<_, MmioRegisterError> {
                let reservation = self
                    .shared
                    .register_gate
                    .reserve(claim_observation_count(&state.core))?;
                let committed = prepare_claim(&mut state.core, reservation.start_sequence())?;
                let generation = if committed.changed() {
                    wake(state);
                    Some(state.generation)
                } else {
                    None
                };
                Ok((committed, reservation, generation))
            })
            .await?;
        let (notification, observations) = committed.into_parts();
        reservation.emit(observations)?;
        if let Some(generation) = generation {
            self.shared.backend.publish(generation);
        }
        Ok(notification)
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
        let (committed, reservation, generation) = self
            .shared
            .backend
            .interact(|state| -> Result<_, MmioRegisterError> {
                let reservation = self
                    .shared
                    .register_gate
                    .reserve(claim_observation_count(&state.core))?;
                let committed = prepare_claim(&mut state.core, reservation.start_sequence())?;
                let generation = if committed.changed() {
                    wake(state);
                    Some(state.generation)
                } else {
                    None
                };
                Ok((committed, reservation, generation))
            })
            .await?;
        let (notification, observations) = committed.into_parts();
        reservation.emit(observations.clone())?;
        if let Some(generation) = generation {
            self.shared.backend.publish(generation);
        }
        Ok((notification, observations))
    }
}

impl RegisterAccess for MmioRegisterEndpoint {
    type Error = MmioRegisterError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
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
        let (register, value) = decode_write(offset, request)?;
        let (prepared, reservation, generation) = self
            .shared
            .backend
            .interact(|state| -> Result<_, MmioRegisterError> {
                let prepared = prepare_write(register, value, offset, request);
                let reservation = self.shared.register_gate.reserve(1)?;
                let changed = prepared.commit(&mut state.core);
                if changed {
                    wake(state);
                }
                let generation = changed.then_some(state.generation);
                Ok((prepared, reservation, generation))
            })
            .await?;
        let observed = prepared.observation(reservation.start_sequence(), AccessOrigin::Direct);
        reservation.emit(vec![observed])?;
        if let Some(generation) = generation {
            self.shared.backend.publish(generation);
        }
        Ok(())
    }
}
