//! Async device IRQ control surface.

use crate::ModelResult;
use crate::irq::mmio::support::{irq_notification_ready, observe_irq_source};
use crate::runtime::{AsyncDeviceHandle, DeviceWait};
use pdma_model_contract::abi::mmio::IrqSourceStatus;
use pdma_model_contract::semantic::EpochIrqSource;
use pdma_model_contract::semantic::irq::{IrqNotification, IrqRoute, IrqSnapshot};

impl AsyncDeviceHandle {
    /// Reports notification readiness without consuming a line assertion or message.
    pub async fn irq_notification_ready(&self) -> bool {
        self.shared
            .backend
            .interact(|state| irq_notification_ready(&state.core))
            .await
    }

    /// Waits for notification readiness without consuming it, or returns false on shutdown.
    pub async fn wait_irq_notification_ready(&self) -> bool {
        loop {
            let (ready, shutdown, generation) = self
                .shared
                .backend
                .interact(|state| {
                    if state.shutdown_requested {
                        return (false, true, state.generation);
                    }
                    if irq_notification_ready(&state.core) {
                        return (true, false, state.generation);
                    }
                    (false, false, state.generation)
                })
                .await;
            if shutdown {
                return false;
            }
            if ready {
                return true;
            }
            self.shared
                .backend
                .wait_for_change(DeviceWait::generation(generation))
                .await;
        }
    }

    /// Returns the complete non-consuming state bits for one IRQ source.
    pub async fn observe_irq_source(&self, source: EpochIrqSource) -> ModelResult<IrqSourceStatus> {
        self.shared
            .backend
            .interact(|state| observe_irq_source(&state.core, source).map(|view| view.status))
            .await
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
        self.with_device(|device| device.ack_irq_source(source))
            .await
    }

    /// Returns the current pending IRQ snapshot, if any.
    pub async fn irq_snapshot(&self) -> Option<IrqSnapshot> {
        self.shared
            .backend
            .interact(|state| state.core.irq_snapshot())
            .await
    }

    /// Waits until the device has an asserted IRQ snapshot.
    pub async fn wait_irq(&self) -> Option<IrqSnapshot> {
        loop {
            let (snapshot, shutdown, generation) = self
                .shared
                .backend
                .interact(|state| {
                    if let Some(snapshot) = state
                        .core
                        .irq_snapshot()
                        .filter(|snapshot| snapshot.asserted())
                    {
                        return (Some(snapshot), false, state.generation);
                    }
                    if state.shutdown_requested {
                        return (None, true, state.generation);
                    }
                    (None, false, state.generation)
                })
                .await;
            if let Some(snapshot) = snapshot {
                return Some(snapshot);
            }
            if shutdown {
                return None;
            }
            self.shared
                .backend
                .wait_for_change(DeviceWait::generation(generation))
                .await;
        }
    }

    /// Waits for one line or message IRQ notification.
    pub async fn wait_irq_notification(&self) -> Option<IrqNotification> {
        loop {
            let (notification, shutdown, generation) = self
                .shared
                .backend
                .interact(|state| {
                    if let Some(notification) = state.core.take_irq_notification() {
                        return (Some(notification), false, state.generation);
                    }
                    if state.shutdown_requested {
                        return (None, true, state.generation);
                    }
                    (None, false, state.generation)
                })
                .await;
            if let Some(notification) = notification {
                return Some(notification);
            }
            if shutdown {
                return None;
            }
            self.shared
                .backend
                .wait_for_change(DeviceWait::generation(generation))
                .await;
        }
    }
}
