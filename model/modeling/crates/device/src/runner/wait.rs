//! Tokio-backed asynchronous wait helpers for runner snapshots.

use super::state::{RunnerState, snapshot, snapshot_with_status};
use super::{AsyncDevice, AsyncDeviceHandle, AsyncRunSnapshot, AsyncStepStatus};

impl AsyncDevice {
    /// Returns the current runner snapshot.
    pub async fn snapshot(&self) -> AsyncRunSnapshot {
        self.handle.snapshot().await
    }

    /// Waits until the runner records any change after the supplied snapshot.
    pub async fn wait_for_change_after(&self, snapshot: AsyncRunSnapshot) -> AsyncRunSnapshot {
        self.handle.wait_for_change_after(snapshot).await
    }

    /// Waits until the runner records more progress than `steps`.
    pub async fn wait_for_progress_after(&self, steps: usize) -> AsyncRunSnapshot {
        self.handle.wait_for_progress_after(steps).await
    }

    /// Waits until the runner records more errors than `errors`.
    pub async fn wait_for_error_after(&self, errors: usize) -> AsyncRunSnapshot {
        self.handle.wait_for_error_after(errors).await
    }

    /// Waits until the runner records more stalls than `stalls`.
    pub async fn wait_for_stall_after(&self, stalls: usize) -> AsyncRunSnapshot {
        self.handle.wait_for_stall_after(stalls).await
    }
}

impl AsyncDeviceHandle {
    /// Returns the current runner snapshot.
    pub async fn snapshot(&self) -> AsyncRunSnapshot {
        let state = self.shared.state.lock().await;
        snapshot(&state)
    }

    /// Waits until the runner records any change after the supplied snapshot.
    pub async fn wait_for_change_after(&self, previous: AsyncRunSnapshot) -> AsyncRunSnapshot {
        self.wait_until(|state| state.generation != previous.generation || state.shutdown_requested)
            .await
    }

    /// Waits until the runner records more progress than `steps`.
    pub async fn wait_for_progress_after(&self, steps: usize) -> AsyncRunSnapshot {
        self.wait_until_snapshot(|state| {
            if state.steps > steps {
                Some(snapshot_with_status(state, AsyncStepStatus::Progressed))
            } else if state.shutdown_requested {
                Some(snapshot(state))
            } else {
                None
            }
        })
        .await
    }

    /// Waits until the runner records more errors than `errors`.
    pub async fn wait_for_error_after(&self, errors: usize) -> AsyncRunSnapshot {
        self.wait_until(|state| state.errors > errors || state.shutdown_requested)
            .await
    }

    /// Waits until the runner records more stalls than `stalls`.
    pub async fn wait_for_stall_after(&self, stalls: usize) -> AsyncRunSnapshot {
        self.wait_until(|state| state.stalls > stalls || state.shutdown_requested)
            .await
    }

    async fn wait_until(
        &self,
        predicate: impl Fn(&RunnerState) -> bool + Send + Sync,
    ) -> AsyncRunSnapshot {
        self.wait_until_snapshot(|state| predicate(state).then(|| snapshot(state)))
            .await
    }

    async fn wait_until_snapshot(
        &self,
        project: impl Fn(&RunnerState) -> Option<AsyncRunSnapshot> + Send + Sync,
    ) -> AsyncRunSnapshot {
        let mut changes = self.shared.changed.subscribe();
        loop {
            let current = {
                let state = self.shared.state.lock().await;
                if let Some(projected) = project(&state) {
                    return projected;
                }
                state.generation
            };
            if *changes.borrow_and_update() != current {
                continue;
            }
            if changes.changed().await.is_err() {
                return self.snapshot().await;
            }
        }
    }
}
