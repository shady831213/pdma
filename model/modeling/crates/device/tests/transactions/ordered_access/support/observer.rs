use std::sync::Mutex;

#[derive(Debug, Default)]
pub(in crate::transactions) struct RecordingObserver(
    Mutex<Vec<hardware_interface_runtime::RegisterObservation>>,
);

impl RecordingObserver {
    pub(in crate::transactions) fn snapshot(
        &self,
    ) -> Vec<hardware_interface_runtime::RegisterObservation> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl hardware_interface_runtime::RegisterObserver for RecordingObserver {
    fn observe(&self, observation: hardware_interface_runtime::RegisterObservation) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}

#[derive(Debug)]
pub(in crate::transactions) struct GatedObserver {
    observations: Mutex<Vec<hardware_interface_runtime::RegisterObservation>>,
    entered: std::sync::Barrier,
    release: std::sync::Barrier,
}

impl GatedObserver {
    pub(in crate::transactions) const fn new() -> Self {
        Self {
            observations: Mutex::new(Vec::new()),
            entered: std::sync::Barrier::new(2),
            release: std::sync::Barrier::new(2),
        }
    }

    pub(in crate::transactions) fn await_observation(&self) {
        self.entered.wait();
    }

    pub(in crate::transactions) fn release_observation(&self) {
        self.release.wait();
    }

    pub(in crate::transactions) fn snapshot(
        &self,
    ) -> Vec<hardware_interface_runtime::RegisterObservation> {
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl hardware_interface_runtime::RegisterObserver for GatedObserver {
    fn observe(&self, observation: hardware_interface_runtime::RegisterObservation) {
        let command = observation.kind == hardware_interface_runtime::AccessKind::Write
            && pdma_model_contract::abi::registers::queue_offset(
                0,
                pdma_model_contract::abi::registers::QueueRegisters::ENABLE.offset(),
            ) == Ok(observation.offset);
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
        if command {
            self.entered.wait();
            self.release.wait();
        }
    }
}
