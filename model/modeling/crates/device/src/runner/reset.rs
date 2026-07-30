use super::bus::BusControlError;
use super::state::{RunnerState, publish, wake};
use super::{AsyncDeviceHandle, SyncDevice};
use pdma_model_contract::abi::{DeviceEpoch, WholeDeviceReset};

impl AsyncDeviceHandle {
    /// Atomically resets device-owned state and advances recovery identity.
    pub async fn reset(&self, request: WholeDeviceReset) -> Result<DeviceEpoch, BusControlError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = reset_runner(&mut state, request);
            if result.is_ok() {
                wake(&mut state);
            }
            (result, state.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }
}

impl SyncDevice {
    /// Atomically resets device-owned state and advances recovery identity.
    pub fn reset(&mut self, request: WholeDeviceReset) -> Result<DeviceEpoch, BusControlError> {
        reset_backend(&mut self.backend, request)
    }
}

fn reset_runner(
    state: &mut RunnerState,
    request: WholeDeviceReset,
) -> Result<DeviceEpoch, BusControlError> {
    let backend_epoch = state.backend.epoch();
    let control_epoch = state.bus_control.epoch();
    if request.expected_epoch() != backend_epoch || control_epoch != backend_epoch {
        return Err(BusControlError::StaleEpoch {
            expected: backend_epoch,
            actual: request.expected_epoch(),
        });
    }
    let next_backend = backend_epoch.next()?;
    let next_control = control_epoch.next()?;
    if next_backend != next_control {
        return Err(BusControlError::InvalidTransaction);
    }
    state.backend.reset(next_backend);
    state.bus_control.reset(next_control);
    Ok(next_backend)
}

fn reset_backend(
    backend: &mut super::backend::DeviceModel,
    request: WholeDeviceReset,
) -> Result<DeviceEpoch, BusControlError> {
    let current = backend.epoch();
    if request.expected_epoch() != current {
        return Err(BusControlError::StaleEpoch {
            expected: current,
            actual: request.expected_epoch(),
        });
    }
    let next = current.next()?;
    backend.reset(next);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::backend::DeviceModel;
    use pdma_model_contract::abi::AbiControlError;
    use pdma_model_contract::ids::BufferId;

    #[test]
    fn epoch_overflow_preserves_complete_runner_state() {
        // Given: matching exhausted epochs beside persistent DMA and lifecycle accounting.
        let exhausted = DeviceEpoch::new(u64::MAX);
        let mut state = RunnerState::new(DeviceModel::queue_port());
        state.backend.reset(exhausted);
        state.bus_control.reset(exhausted);
        let region = state.dma_space.allocate(1).expect("DMA allocation");
        state
            .dma_space
            .write_object(region.object(), 0, &[0x6d])
            .expect("DMA write");
        state.steps = 7;
        state.errors = 3;
        state.stalls = 2;
        state.shutdown_requested = true;
        let generation = state.generation;

        // When: reset cannot checked-advance the current epoch.
        let result = reset_runner(&mut state, WholeDeviceReset::new(exhausted));

        // Then: the typed overflow leaves identity, DMA, counters, and lifecycle unchanged.
        assert_eq!(
            result,
            Err(BusControlError::Abi(AbiControlError::DeviceEpochOverflow {
                current: u64::MAX
            }))
        );
        assert_eq!(state.backend.epoch(), exhausted);
        assert_eq!(state.bus_control.epoch(), exhausted);
        assert_eq!(
            state.dma_space.read_object(BufferId::new(0), 0, 1),
            Ok(vec![0x6d])
        );
        assert_eq!((state.steps, state.errors, state.stalls), (7, 3, 2));
        assert!(state.shutdown_requested);
        assert_eq!(state.generation, generation);
    }
}
