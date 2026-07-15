//! Async device IRQ control surface.

use super::AsyncDeviceHandle;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::irq::{IrqNotification, IrqRoute, IrqSnapshot, IrqSource};

impl AsyncDeviceHandle {
    /// Configures the delivery route of an inactive PDMA-local IRQ source.
    pub async fn configure_irq_route(&self, source: IrqSource, route: IrqRoute) -> ModelResult<()> {
        self.with_backend(|backend| backend.configure_irq_route(source, route))
            .await
    }

    /// Enables a PDMA-local IRQ source.
    pub async fn enable_irq_source(&self, source: IrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.enable_irq_source(source))
            .await
    }

    /// Disables a PDMA-local IRQ source.
    pub async fn disable_irq_source(&self, source: IrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.disable_irq_source(source))
            .await
    }

    /// Masks a PDMA-local IRQ source without clearing pending state.
    pub async fn mask_irq_source(&self, source: IrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.mask_irq_source(source))
            .await
    }

    /// Unmasks a PDMA-local IRQ source.
    pub async fn unmask_irq_source(&self, source: IrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.unmask_irq_source(source))
            .await
    }

    /// Acknowledges a source, retaining a Line cause while source records remain.
    pub async fn ack_irq_source(&self, source: IrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.ack_irq_source(source))
            .await
    }

    /// Returns the current pending IRQ snapshot, if any.
    pub async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.shared.state.lock().await.backend.irq_snapshot()
    }

    /// Waits until the device has an asserted IRQ snapshot.
    pub async fn wait_irq(&self) -> Option<IrqSnapshot> {
        let mut changes = self.shared.changed.subscribe();
        loop {
            let current = {
                let state = self.shared.state.lock().await;
                if let Some(snapshot) = state
                    .backend
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
                if let Some(notification) = state.backend.take_irq_notification() {
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
                return self
                    .shared
                    .state
                    .lock()
                    .await
                    .backend
                    .take_irq_notification();
            }
        }
    }
}
