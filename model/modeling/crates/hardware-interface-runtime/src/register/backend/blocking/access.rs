use super::{
    BlockingObservationBackend, BlockingState, GateFault, ObservationGateError, ObservationOrder,
    order_error,
};
use crate::register::ObservationBackend;
use std::sync::MutexGuard;

impl BlockingObservationBackend {
    pub(super) fn lock_state(&self) -> Result<MutexGuard<'_, BlockingState>, ObservationGateError> {
        if self.state.is_active() {
            self.deferred_fault
                .get_or_init(|| GateFault::SynchronizationPoisoned);
            return Err(ObservationGateError::Faulted {
                reason: GateFault::SynchronizationPoisoned,
            });
        }
        self.state.lock_checked().map_err(|poisoned| {
            let mut state = poisoned.into_inner();
            self.apply_deferred(&mut state);
            state.order.fault(GateFault::SynchronizationPoisoned);
            let error = order_error(state.order.fault_error());
            drop(state);
            self.changed.notify_all();
            error
        })
    }

    pub(super) fn fault(&self, reason: GateFault) {
        if self.state.is_active() {
            // The caller owns this mutex; its closure guard records the fault before unlocking.
            self.deferred_fault.get_or_init(|| reason);
            return;
        }
        let mut state = match self.state.lock_checked() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                self.apply_deferred(&mut state);
                state.order.fault(GateFault::SynchronizationPoisoned);
                state
            }
        };
        state.order.fault(reason);
        state.emitter_thread = None;
        drop(state);
        self.changed.notify_all();
    }

    fn apply_deferred(&self, state: &mut BlockingState) {
        if let Some(reason) = self.deferred_fault.get() {
            state.order.fault(*reason);
            state.emitter_thread = None;
        }
    }
}

struct ClosureAccess<'a> {
    backend: &'a BlockingObservationBackend,
    state: MutexGuard<'a, BlockingState>,
}

impl Drop for ClosureAccess<'_> {
    fn drop(&mut self) {
        self.backend.apply_deferred(&mut self.state);
        self.backend.changed.notify_all();
    }
}

impl ObservationBackend for BlockingObservationBackend {
    type Error = ObservationGateError;

    fn inspect<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order ObservationOrder) -> R,
    ) -> Result<R, Self::Error> {
        self.mutate(|order| operation(order))
    }

    fn mutate<R>(
        &self,
        operation: impl for<'order> FnOnce(&'order mut ObservationOrder) -> R,
    ) -> Result<R, Self::Error> {
        let mut access = ClosureAccess {
            backend: self,
            state: self.lock_state()?,
        };
        let _entry = self
            .state
            .enter()
            .map_err(|_| super::super::storage_error())?;
        Ok(operation(&mut access.state.order))
    }
}
