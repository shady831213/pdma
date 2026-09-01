use super::{AsyncDevice, DeviceCore, RunnerShared, publish};
use crate::abi::endpoint::{MmioRegisterError, prepare_claim, prepare_read, prepare_write};
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
        let committed = {
            let mut state = self.shared.state.lock().await;
            prepare_claim(&mut state)?
        };
        let generation = committed.generation();
        let notification = committed.emit()?;
        if let Some(generation) = generation {
            publish(&self.shared, generation);
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
        let committed = {
            let mut state = self.shared.state.lock().await;
            prepare_claim(&mut state)?
        };
        let generation = committed.generation();
        let result = committed.emit_observed()?;
        if let Some(generation) = generation {
            publish(&self.shared, generation);
        }
        Ok(result)
    }
}

impl RegisterAccess for MmioRegisterEndpoint {
    type Error = MmioRegisterError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        let register = decode_read(offset, response.len() as u128)?;
        let (prepared, reservation) = {
            let state = self.shared.state.lock().await;
            let prepared = prepare_read(&state, register, offset);
            let reservation = state.register_gate.reserve(1)?;
            drop(state);
            (prepared, reservation)
        };
        let observed = prepared.observation(reservation.start_sequence());
        response.copy_from_slice(&prepared.response);
        reservation.emit(vec![observed])?;
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        let (register, value) = decode_write(offset, request)?;
        let (prepared, reservation, generation) = {
            let mut state = self.shared.state.lock().await;
            let prepared = prepare_write(register, value, offset, request);
            let reservation = state.register_gate.reserve(1)?;
            let previous = state.generation;
            prepared.commit(&mut state);
            let generation = (state.generation != previous).then_some(state.generation);
            drop(state);
            (prepared, reservation, generation)
        };
        let observed = prepared.observation(reservation.start_sequence(), AccessOrigin::Direct);
        reservation.emit(vec![observed])?;
        if let Some(generation) = generation {
            publish(&self.shared, generation);
        }
        Ok(())
    }
}
