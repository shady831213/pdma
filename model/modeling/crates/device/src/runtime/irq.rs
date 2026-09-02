//! Async device IRQ control surface.

use super::AsyncDeviceHandle;
use crate::ModelResult;
use crate::interface::mmio::irq::support::{irq_notification_ready, observe_irq_source};
use pdma_model_contract::abi::mmio::IrqSourceStatus;
use pdma_model_contract::semantic::EpochIrqSource;
use pdma_model_contract::semantic::irq::{IrqNotification, IrqRoute, IrqSnapshot};

impl AsyncDeviceHandle {
    /// Reports notification readiness without consuming a line assertion or message.
    pub async fn irq_notification_ready(&self) -> bool {
        let state = self.shared.state.lock().await;
        irq_notification_ready(&state.core)
    }

    /// Waits for notification readiness without consuming it, or returns false on shutdown.
    pub async fn wait_irq_notification_ready(&self) -> bool {
        let mut changes = self.shared.changed.subscribe();
        loop {
            let current = {
                let state = self.shared.state.lock().await;
                if state.shutdown_requested {
                    return false;
                }
                if irq_notification_ready(&state.core) {
                    return true;
                }
                state.generation
            };
            if *changes.borrow_and_update() != current {
                continue;
            }
            if changes.changed().await.is_err() {
                return false;
            }
        }
    }

    /// Returns the complete non-consuming state bits for one IRQ source.
    pub async fn observe_irq_source(&self, source: EpochIrqSource) -> ModelResult<IrqSourceStatus> {
        let state = self.shared.state.lock().await;
        observe_irq_source(&state.core, source).map(|view| view.status)
    }
    /// Configures the delivery route of an inactive PDMA-local IRQ source.
    pub async fn configure_irq_route(
        &self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.with_device(|device| device.configure_irq_route(source, route))
            .await
    }

    /// Enables a PDMA-local IRQ source.
    pub async fn enable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_device(|device| device.enable_irq_source(source))
            .await
    }

    /// Disables a PDMA-local IRQ source.
    pub async fn disable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_device(|device| device.disable_irq_source(source))
            .await
    }

    /// Masks a PDMA-local IRQ source without clearing pending state.
    pub async fn mask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_device(|device| device.mask_irq_source(source))
            .await
    }

    /// Unmasks a PDMA-local IRQ source.
    pub async fn unmask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_device(|device| device.unmask_irq_source(source))
            .await
    }

    /// Acknowledges a source, retaining a Line cause while source records remain.
    pub async fn ack_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = {
                let state = &mut *state;
                state.core.ack_irq_source(source)
            };
            super::wake(&mut state);
            (result, state.generation)
        };
        super::publish(&self.shared, generation);
        result
    }

    /// Returns the current pending IRQ snapshot, if any.
    pub async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.shared.state.lock().await.core.irq_snapshot()
    }

    /// Waits until the device has an asserted IRQ snapshot.
    pub async fn wait_irq(&self) -> Option<IrqSnapshot> {
        let mut changes = self.shared.changed.subscribe();
        loop {
            let current = {
                let state = self.shared.state.lock().await;
                if let Some(snapshot) = state
                    .core
                    .irq_snapshot()
                    .filter(|snapshot| snapshot.asserted())
                {
                    return Some(snapshot);
                }
                if state.shutdown_requested {
                    return None;
                }
                state.generation
            };
            if *changes.borrow_and_update() != current {
                continue;
            }
            if changes.changed().await.is_err() {
                return self
                    .irq_snapshot()
                    .await
                    .filter(|snapshot| snapshot.asserted());
            }
        }
    }

    /// Waits for one line or message IRQ notification.
    pub async fn wait_irq_notification(&self) -> Option<IrqNotification> {
        let mut changes = self.shared.changed.subscribe();
        loop {
            let current = {
                let mut state = self.shared.state.lock().await;
                if let Some(notification) = state.core.take_irq_notification() {
                    return Some(notification);
                }
                if state.shutdown_requested {
                    return None;
                }
                state.generation
            };
            if *changes.borrow_and_update() != current {
                continue;
            }
            if changes.changed().await.is_err() {
                return self.shared.state.lock().await.core.take_irq_notification();
            }
        }
    }
}
