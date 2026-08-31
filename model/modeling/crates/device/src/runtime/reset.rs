use super::AsyncDeviceHandle;
#[cfg(test)]
use super::state::DeviceCore;
use super::state::{publish, wake};
use crate::abi::ring::PdmaControlError;
use pdma_model_contract::semantic::{DeviceEpoch, WholeDeviceReset};

impl AsyncDeviceHandle {
    /// Atomically resets device-owned state and advances recovery identity.
    pub async fn reset(&self, request: WholeDeviceReset) -> Result<DeviceEpoch, PdmaControlError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result =
                crate::abi::mmio::reset::execute(&mut state, request.expected_epoch().raw())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::mmio::reset::{self as mmio_reset, ResetOutcome};
    use pdma_model_contract::abi::mmio::{MmioResult, MmioResultDetails};
    use pdma_model_contract::semantic::{SemanticError, ids::BufferId};

    #[test]
    fn raw_reset_seam_maps_success_and_stale_epoch_exactly() {
        // Given: a runner at its initial epoch.
        let mut state = DeviceCore::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
            std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
        );

        // When: the raw reset seam receives the current epoch.
        let success = mmio_reset::execute(&mut state, 0);

        // Then: it advances both identities and returns the new epoch.
        assert_eq!(success, ResetOutcome::Success(DeviceEpoch::new(1)));
        assert_eq!(state.device.execution.epoch(), DeviceEpoch::new(1));
        assert_eq!(state.device.queue_control.epoch(), DeviceEpoch::new(1));

        // When: the same raw epoch is submitted again.
        let stale_outcome = mmio_reset::execute(&mut state, 0);

        // Then: exact stale details are returned before either identity changes.
        assert_eq!(
            stale_outcome,
            ResetOutcome::Rejected(
                PdmaControlError::StaleEpoch {
                    expected: DeviceEpoch::new(1),
                    actual: DeviceEpoch::initial(),
                },
                MmioResultDetails {
                    result: MmioResult::StaleEpoch,
                    details: [1, 0, 0],
                }
            )
        );
        assert_eq!(state.device.execution.epoch(), DeviceEpoch::new(1));
        assert_eq!(state.device.queue_control.epoch(), DeviceEpoch::new(1));
    }

    #[test]
    fn epoch_overflow_preserves_complete_runner_state() {
        // Given: matching exhausted epochs beside persistent DMA and lifecycle accounting.
        let exhausted = DeviceEpoch::new(u64::MAX);
        let mut state = DeviceCore::new(
            std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
            pdma_model_contract::semantic::ResourceConfig::default(),
            std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
        );
        state.device.execution.reset(exhausted);
        state.device.queue_control.reset(exhausted);
        let region = state.device.dma.allocate(1).expect("DMA allocation");
        state
            .device
            .dma
            .write_object(region.object(), 0, &[0x6d])
            .expect("DMA write");
        state.steps = 7;
        state.errors = 3;
        state.stalls = 2;
        state.shutdown_requested = true;
        let generation = state.generation;

        // When: reset cannot checked-advance the current epoch.
        let result = state.reset_runner(WholeDeviceReset::new(exhausted));

        // Then: the typed overflow leaves identity, DMA, counters, and lifecycle unchanged.
        assert_eq!(
            result,
            Err(PdmaControlError::Semantic(
                SemanticError::DeviceEpochOverflow { current: u64::MAX }
            ))
        );
        assert_eq!(state.device.execution.epoch(), exhausted);
        assert_eq!(state.device.queue_control.epoch(), exhausted);
        assert_eq!(
            state.device.dma.read_object(BufferId::new(0), 0, 1),
            Ok(vec![0x6d])
        );
        assert_eq!((state.steps, state.errors, state.stalls), (7, 3, 2));
        assert!(state.shutdown_requested);
        assert_eq!(state.generation, generation);

        // When: the raw reset seam receives the same exhausted expected epoch.
        let raw = mmio_reset::execute(&mut state, u64::MAX);

        // Then: it reports exact overflow details and still commits no mutation.
        assert_eq!(
            raw,
            ResetOutcome::Rejected(
                PdmaControlError::Semantic(SemanticError::DeviceEpochOverflow {
                    current: u64::MAX,
                }),
                pdma_model_contract::abi::mmio::MmioResultDetails {
                    result: pdma_model_contract::abi::mmio::MmioResult::DeviceEpochOverflow,
                    details: [u64::MAX, 0, 0],
                }
            )
        );
        assert_eq!(state.device.execution.epoch(), exhausted);
        assert_eq!(state.device.queue_control.epoch(), exhausted);
        assert_eq!(state.generation, generation);
    }
}
