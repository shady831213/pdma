//! Async device IRQ control surface.

use crate::ModelResult;
use crate::runtime::{AsyncDeviceHandle, DeviceWait};
use pdma_model_contract::semantic::DeviceEpoch;
use pdma_model_contract::semantic::irq::{IrqNotification, IrqSnapshot};

#[cfg(test)]
mod tests;

impl AsyncDeviceHandle {
    /// Takes one available hardware IRQ output after validating its owning epoch.
    ///
    /// Empty output returns immediately; dropping while awaiting the backend lock consumes nothing.
    pub async fn take_irq_notification_at_epoch(
        &self,
        epoch: DeviceEpoch,
    ) -> ModelResult<Option<IrqNotification>> {
        self.shared
            .backend
            .commit(
                |state| {
                    state.core.validate_epoch(epoch)?;
                    let notification = state.core.take_irq_notification();
                    Ok((notification, notification.is_some()))
                },
                Ok,
            )
            .await
    }

    /// Waits without consumption, rejecting reset epochs before inspecting readiness.
    pub async fn wait_irq_notification_ready_at_epoch(
        &self,
        epoch: DeviceEpoch,
    ) -> ModelResult<bool> {
        loop {
            let (ready, shutdown, generation) = self
                .shared
                .backend
                .interact(|state| -> ModelResult<_> {
                    state.core.validate_epoch(epoch)?;
                    Ok((
                        state.core.irq_notification_ready(),
                        state.shutdown_requested,
                        state.generation,
                    ))
                })
                .await?;
            if shutdown {
                return Ok(false);
            }
            if ready {
                return Ok(true);
            }
            self.shared
                .backend
                .wait_for_change(DeviceWait::generation(generation))
                .await;
        }
    }

    /// Reports notification readiness without consuming a line assertion or message.
    pub async fn irq_notification_ready(&self) -> bool {
        self.shared
            .backend
            .interact(|state| state.core.irq_notification_ready())
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
                    if state.core.irq_notification_ready() {
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
}
