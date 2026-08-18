//! Async device IRQ control surface.

use super::AsyncDeviceHandle;
use super::backend::DeviceModel;
use super::state::DeviceCore;
use crate::ModelResult;
use pdma_model_contract::abi::mmio::{
    IrqCommand, IrqSourceStatus, IrqStateBit, MmioResult, MmioResultDetails,
};
use pdma_model_contract::semantic::EpochIrqSource;
use pdma_model_contract::semantic::irq::{
    IrqLine, IrqNotification, IrqRoute, IrqSnapshot, IrqSource, IrqVector,
};
use pdma_model_contract::semantic::{CompletionRingId, DeviceEpoch, QueueId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runner) struct IrqSourceView {
    pub(in crate::runner) source: EpochIrqSource,
    pub(in crate::runner) route: IrqRoute,
    pub(in crate::runner) status: IrqSourceStatus,
    pub(in crate::runner) pending_messages: usize,
}

pub(in crate::runner) fn observe_irq_source(
    state: &DeviceCore,
    source: EpochIrqSource,
) -> ModelResult<IrqSourceView> {
    let (observed, records_pending) = match &state.backend {
        DeviceModel::QueuePort(device) => device.observe_irq_source(&state.bus_control, source)?,
    };
    let mut bits = IrqStateBit::Present.mask();
    for (set, bit) in [
        (observed.enabled(), IrqStateBit::Enabled),
        (observed.masked(), IrqStateBit::Masked),
        (observed.cause_pending(), IrqStateBit::CausePending),
        (observed.deliverable(), IrqStateBit::Deliverable),
        (observed.line_asserted(), IrqStateBit::LineAsserted),
        (records_pending, IrqStateBit::RecordsPending),
    ] {
        if set {
            bits |= bit.mask();
        }
    }
    let status =
        IrqSourceStatus::try_from(bits).map_err(|_| crate::ModelError::DmaSubmissionState)?;
    Ok(IrqSourceView {
        source,
        route: observed.route,
        status,
        pending_messages: observed.pending_messages,
    })
}

pub(in crate::runner) fn irq_notification_ready(state: &DeviceCore) -> bool {
    match &state.backend {
        DeviceModel::QueuePort(device) => device.irq_notification_ready(),
    }
}

pub(in crate::runner) fn decode_source(epoch: u64, kind: u32, id: u64) -> Option<EpochIrqSource> {
    let raw = usize::try_from(id).ok()?;
    let source = match kind {
        0 => IrqSource::Completion(CompletionRingId::new(raw)),
        1 => IrqSource::Event(QueueId::new(raw)),
        _ => return None,
    };
    Some(EpochIrqSource::new(DeviceEpoch::new(epoch), source))
}

pub(in crate::runner) fn decode_route(kind: u32, vector: u64) -> Option<IrqRoute> {
    let vector = IrqVector::new(usize::try_from(vector).ok()?);
    match kind {
        0 => Some(IrqRoute::Line(IrqLine::new(vector))),
        1 => Some(IrqRoute::Message(vector)),
        _ => None,
    }
}

pub(in crate::runner) fn encode_source(source: IrqSource) -> (u32, u64) {
    match source {
        IrqSource::Completion(id) => (0, usize_to_u64(id.raw())),
        IrqSource::Event(id) => (1, usize_to_u64(id.raw())),
    }
}

pub(in crate::runner) fn encode_route(route: IrqRoute) -> (u32, u64) {
    match route {
        IrqRoute::Line(line) => (0, usize_to_u64(line.vector().raw())),
        IrqRoute::Message(vector) => (1, usize_to_u64(vector.raw())),
    }
}

pub(in crate::runner) fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).map_or(u64::MAX, core::convert::identity)
}

pub(in crate::runner) fn map_mmio_error(
    error: crate::ModelError,
    command: IrqCommand,
    source: EpochIrqSource,
) -> MmioResultDetails {
    let (kind, id) = encode_source(source.source());
    let (result, details) = match error {
        crate::ModelError::StaleDeviceEpoch { expected, actual } => {
            (MmioResult::StaleEpoch, [expected.raw(), actual.raw(), 0])
        }
        crate::ModelError::UnknownIrqSource(_) => {
            (MmioResult::InvalidIrqSource, [u64::from(kind), id, 0])
        }
        crate::ModelError::IrqSourceActive(_) => {
            (MmioResult::IrqSourceActive, [u64::from(kind), id, 0])
        }
        _ => (
            MmioResult::InvalidTransaction,
            [(2_u64 << 32) | u64::from(command.code()), 0, 0],
        ),
    };
    MmioResultDetails { result, details }
}

impl AsyncDeviceHandle {
    /// Reports notification readiness without consuming a line assertion or message.
    pub async fn irq_notification_ready(&self) -> bool {
        let state = self.shared.state.lock().await;
        irq_notification_ready(&state)
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
                if irq_notification_ready(&state) {
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
        observe_irq_source(&state, source).map(|view| view.status)
    }
    /// Configures the delivery route of an inactive PDMA-local IRQ source.
    pub async fn configure_irq_route(
        &self,
        source: EpochIrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        self.with_backend(|backend| backend.configure_irq_route(source, route))
            .await
    }

    /// Enables a PDMA-local IRQ source.
    pub async fn enable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.enable_irq_source(source))
            .await
    }

    /// Disables a PDMA-local IRQ source.
    pub async fn disable_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.disable_irq_source(source))
            .await
    }

    /// Masks a PDMA-local IRQ source without clearing pending state.
    pub async fn mask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.mask_irq_source(source))
            .await
    }

    /// Unmasks a PDMA-local IRQ source.
    pub async fn unmask_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        self.with_backend(|backend| backend.unmask_irq_source(source))
            .await
    }

    /// Acknowledges a source, retaining a Line cause while source records remain.
    pub async fn ack_irq_source(&self, source: EpochIrqSource) -> ModelResult<()> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = {
                let state = &mut *state;
                state.backend.ack_irq_source(&state.bus_control, source)
            };
            super::wake(&mut state);
            (result, state.generation)
        };
        super::publish(&self.shared, generation);
        result
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
