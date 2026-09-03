use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use crate::{LocalStorage, StorageBackend, StorageError};

use super::{ObservationGateError, order_error, storage_error};
use crate::register::{
    GateFault, ObservationBackend, ObservationEmissionReceipt, ObservationOrder,
    ObservationOrderError, ObservationReservationReceipt, RegisterObservation,
    SharedRegisterObserver,
};

/// Clone-shared single-thread observation policy that never waits.
#[derive(Clone, Debug)]
pub struct LocalObservationBackend {
    observer: SharedRegisterObserver,
    state: LocalStorage<ObservationOrder>,
}

impl LocalObservationBackend {
    /// Creates a local policy whose first reservation starts at sequence zero.
    pub fn new(observer: SharedRegisterObserver) -> Self {
        Self::new_at(observer, 0)
    }

    /// Creates a local policy with an explicit first sequence.
    pub fn new_at(observer: SharedRegisterObserver, next_sequence: u64) -> Self {
        Self {
            observer,
            state: LocalStorage::new(ObservationOrder::new_at(next_sequence)),
        }
    }

    /// Returns the next sequence that a successful reservation would receive.
    pub fn next_sequence(&self) -> Result<u64, ObservationGateError> {
        self.state
            .inspect(ObservationOrder::next_sequence)
            .map_err(|_| storage_error())?
            .map_err(order_error)
    }

    /// Reserves a nonempty contiguous range at a consumer-owned commit point.
    pub fn reserve(
        &self,
        count: usize,
    ) -> Result<LocalObservationReservation, ObservationGateError> {
        let receipt = self
            .state
            .mutate(|order| order.reserve(count))
            .map_err(|_| storage_error())?
            .map_err(order_error)?;
        Ok(LocalObservationReservation {
            backend: self.clone(),
            receipt,
            armed: true,
        })
    }

    fn validate(
        &self,
        receipt: &mut ObservationReservationReceipt,
        observations: &[RegisterObservation],
    ) -> Result<ObservationEmissionReceipt, ObservationGateError> {
        self.state
            .mutate(|order| {
                order.validate_with(receipt, observations.len(), |index| {
                    observations.get(index).map(|item| item.sequence)
                })
            })
            .map_err(|_| storage_error())?
            .map_err(order_error)
    }

    fn fault(&self, reason: GateFault) {
        let _ = self.state.mutate(|order| order.fault(reason));
    }

    fn abandon(&self, receipt: &mut ObservationReservationReceipt) {
        let _ = self.state.mutate(|order| order.abandon(receipt));
    }
}

impl ObservationBackend for LocalObservationBackend {
    type Error = StorageError;

    fn inspect<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order ObservationOrder) -> R,
    ) -> Result<R, Self::Error> {
        self.state.inspect(operation)
    }

    fn mutate<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order mut ObservationOrder) -> R,
    ) -> Result<R, Self::Error> {
        self.state.mutate(operation)
    }
}

/// Affine contiguous range reserved from a [`LocalObservationBackend`].
#[derive(Debug)]
pub struct LocalObservationReservation {
    backend: LocalObservationBackend,
    receipt: ObservationReservationReceipt,
    armed: bool,
}

impl LocalObservationReservation {
    /// Returns the first reserved sequence.
    pub const fn start_sequence(&self) -> u64 {
        self.receipt.start_sequence()
    }

    /// Emits the exact reserved range synchronously without waiting.
    pub fn emit(
        mut self,
        observations: Vec<RegisterObservation>,
    ) -> Result<(), ObservationGateError> {
        let mut emission = match self.backend.validate(&mut self.receipt, &observations) {
            Ok(emission) => emission,
            Err(error) => {
                self.armed = false;
                return Err(error);
            }
        };
        self.armed = false;
        for observation in observations {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                self.backend.observer.observe(observation);
            })) {
                self.backend.fault(GateFault::ObserverPanicked);
                resume_unwind(payload);
            }
        }
        let result = self
            .backend
            .state
            .mutate(|order| order.advance(&mut emission))
            .map_err(|_| storage_error())?;
        if matches!(
            result,
            Err(ObservationOrderError::EmissionOutOfOrder { .. })
        ) {
            self.backend.fault(GateFault::InvalidEmission);
        }
        result.map_err(order_error)
    }
}

impl Drop for LocalObservationReservation {
    fn drop(&mut self) {
        if self.armed {
            self.armed = false;
            self.backend.abandon(&mut self.receipt);
        }
    }
}
