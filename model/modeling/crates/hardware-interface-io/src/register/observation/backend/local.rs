use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::{
    cell::{BorrowMutError, Cell, RefCell},
    rc::Rc,
};

use super::{ObservationGateError, order_error, storage_error};
use crate::register::observation::{
    GateFault, ObservationBackend, ObservationEmissionReceipt, ObservationOrder,
    ObservationOrderError, ObservationReservationReceipt, RegisterObservation,
    SharedRegisterObserver,
};

/// Clone-shared single-thread observation policy that never waits.
///
/// Ordering state is borrowed synchronously. Direct [`ObservationBackend`] access
/// returns [`BorrowMutError`] on reentry; gate operations translate a rejected
/// borrow into a retained [`GateFault::SynchronizationPoisoned`] fault.
#[derive(Clone, Debug)]
pub struct LocalObservationGate {
    observer: SharedRegisterObserver,
    state: Rc<RefCell<ObservationOrder>>,
    deferred_fault: Rc<Cell<Option<GateFault>>>,
}

impl LocalObservationGate {
    /// Creates a local policy whose first reservation starts at sequence zero.
    pub fn new(observer: SharedRegisterObserver) -> Self {
        Self::new_at(observer, 0)
    }

    /// Creates a local policy with an explicit first sequence.
    pub fn new_at(observer: SharedRegisterObserver, next_sequence: u64) -> Self {
        Self {
            observer,
            state: Rc::new(RefCell::new(ObservationOrder::new_at(next_sequence))),
            deferred_fault: Rc::new(Cell::new(None)),
        }
    }

    /// Returns the next sequence that a successful reservation would receive.
    pub fn next_sequence(&self) -> Result<u64, ObservationGateError> {
        self.mutate_order(|order| order.next_sequence())
            .map_err(|_| self.rejected_access())?
            .map_err(order_error)
    }

    /// Reserves a nonempty contiguous range at a consumer-owned commit point.
    pub fn reserve(
        &self,
        count: usize,
    ) -> Result<LocalObservationReservation, ObservationGateError> {
        let receipt = self
            .mutate_order(|order| order.reserve(count))
            .map_err(|_| self.rejected_access())?
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
        self.mutate_order(|order| {
            if order.next_emission_sequence()? != receipt.start_sequence() {
                order.fault(GateFault::InvalidEmission);
                return Err(ObservationOrderError::Faulted {
                    reason: GateFault::InvalidEmission,
                });
            }
            order.validate_with(receipt, observations.len(), |index| {
                observations.get(index).map(|item| item.sequence)
            })
        })
        .map_err(|_| self.rejected_access())?
        .map_err(order_error)
    }

    fn rejected_access(&self) -> ObservationGateError {
        self.fault(GateFault::SynchronizationPoisoned);
        storage_error()
    }

    fn fault(&self, reason: GateFault) {
        self.deferred_fault
            .set(self.deferred_fault.get().or(Some(reason)));
        match self.mutate_order(|_| ()) {
            // The active access guard applies the retained fault on exit, including unwind.
            Ok(()) | Err(_) => {}
        }
    }

    fn mutate_order<R>(
        &self,
        operation: impl FnOnce(&mut ObservationOrder) -> R,
    ) -> Result<R, BorrowMutError> {
        let mut order = self.state.try_borrow_mut()?;
        let mut access = OrderAccess {
            order: &mut order,
            deferred_fault: &self.deferred_fault,
        };
        access.apply_fault();
        Ok(operation(access.order))
    }
}

struct OrderAccess<'a> {
    order: &'a mut ObservationOrder,
    deferred_fault: &'a Cell<Option<GateFault>>,
}

impl OrderAccess<'_> {
    fn apply_fault(&mut self) {
        if let Some(reason) = self.deferred_fault.take() {
            self.order.fault(reason);
        }
    }
}

impl Drop for OrderAccess<'_> {
    fn drop(&mut self) {
        self.apply_fault();
    }
}

impl ObservationBackend for LocalObservationGate {
    type Error = BorrowMutError;

    fn inspect<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order ObservationOrder) -> R,
    ) -> Result<R, Self::Error> {
        self.mutate_order(|order| operation(order))
    }

    fn mutate<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order mut ObservationOrder) -> R,
    ) -> Result<R, Self::Error> {
        self.mutate_order(operation)
    }
}

/// Affine contiguous range reserved from a [`LocalObservationGate`].
#[derive(Debug)]
pub struct LocalObservationReservation {
    backend: LocalObservationGate,
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
            self.backend
                .mutate_order(|order| order.next_emission_sequence())
                .map_err(|_| self.backend.rejected_access())?
                .map_err(order_error)?;
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                self.backend.observer.observe(observation);
            })) {
                self.backend.fault(GateFault::ObserverPanicked);
                resume_unwind(payload);
            }
        }
        let result = self
            .backend
            .mutate_order(|order| order.advance(&mut emission))
            .map_err(|_| self.backend.rejected_access())?;
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
            self.backend.fault(GateFault::ReservationDropped);
        }
    }
}
