use super::bus::BusControlError;
use super::state::{RunnerState, publish, wake};
use super::{AsyncDeviceHandle, SyncDevice};
use pdma_model_contract::{DeviceEpoch, WholeDeviceReset};

impl AsyncDeviceHandle {
    /// Atomically resets device-owned state and advances recovery identity.
    pub async fn reset(&self, request: WholeDeviceReset) -> Result<DeviceEpoch, BusControlError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = super::mmio::reset::execute(&mut state, request.expected_epoch().raw())
                .typed_result();
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

pub(in crate::runner) fn reset_runner(
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
    super::mmio::reset::register_banks(state);
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
    use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};
    use pdma_model_contract::ids::BufferId;

    #[test]
    fn raw_reset_seam_maps_success_and_stale_epoch_exactly() {
        // Given: a runner at its initial epoch.
        let mut state = RunnerState::new(DeviceModel::queue_port());

        // When: the raw reset seam receives the current epoch.
        let success = super::super::mmio::reset::execute(&mut state, 0);

        // Then: it advances both identities and returns the new epoch.
        assert_eq!(
            success,
            super::super::mmio::reset::ResetOutcome::Success(DeviceEpoch::new(1))
        );
        assert_eq!(state.backend.epoch(), DeviceEpoch::new(1));
        assert_eq!(state.bus_control.epoch(), DeviceEpoch::new(1));

        // When: the same raw epoch is submitted again.
        let stale_outcome = super::super::mmio::reset::execute(&mut state, 0);

        // Then: exact stale details are returned before either identity changes.
        assert_eq!(
            stale_outcome,
            super::super::mmio::reset::ResetOutcome::Rejected(
                BusControlError::StaleEpoch {
                    expected: DeviceEpoch::new(1),
                    actual: DeviceEpoch::initial(),
                },
                MmioResultDetails {
                    result: MmioResult::StaleEpoch,
                    details: [1, 0, 0],
                }
            )
        );
        assert_eq!(state.backend.epoch(), DeviceEpoch::new(1));
        assert_eq!(state.bus_control.epoch(), DeviceEpoch::new(1));
    }

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

        // When: the raw reset seam receives the same exhausted expected epoch.
        let raw = super::super::mmio::reset::execute(&mut state, u64::MAX);

        // Then: it reports exact overflow details and still commits no mutation.
        assert_eq!(
            raw,
            super::super::mmio::reset::ResetOutcome::Rejected(
                BusControlError::Abi(AbiControlError::DeviceEpochOverflow { current: u64::MAX }),
                pdma_model_contract::abi::mmio::MmioResultDetails {
                    result: pdma_model_contract::abi::mmio::MmioResult::DeviceEpochOverflow,
                    details: [u64::MAX, 0, 0],
                }
            )
        );
        assert_eq!(state.backend.epoch(), exhausted);
        assert_eq!(state.bus_control.epoch(), exhausted);
        assert_eq!(state.generation, generation);
    }
}
