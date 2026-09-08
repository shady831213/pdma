use super::IrqSourceObservation;
use crate::{ModelError, ModelResult};
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochIrqSource, IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification,
    IrqRoute, IrqSnapshot, IrqSource,
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct PdmaIrqController {
    pub(crate) registers: pdma_model_contract::abi::mmio::IrqRegistersValues,
    epoch: DeviceEpoch,
    line: IrqLine,
    sources: BTreeMap<IrqSource, PdmaIrqSourceState>,
}

#[derive(Debug, Clone, Copy)]
struct PdmaIrqSourceState {
    source: IrqSource,
    route: IrqRoute,
    enabled: bool,
    masked: bool,
    cause_pending: bool,
    pending_messages: usize,
    next_message_id: usize,
}

impl PdmaIrqController {
    pub(crate) fn new(epoch: DeviceEpoch, line: IrqLine) -> Self {
        Self {
            registers: pdma_model_contract::abi::mmio::IrqRegistersValues::default(),
            epoch,
            line,
            sources: BTreeMap::new(),
        }
    }

    pub(crate) fn enable(&mut self, source: IrqSource) {
        self.source_state_mut(source).enabled = true;
    }

    pub(crate) fn configure_route(
        &mut self,
        source: IrqSource,
        route: IrqRoute,
    ) -> ModelResult<()> {
        let state = self.source_state_mut(source);
        if state.is_active() {
            return Err(ModelError::IrqSourceActive(source));
        }
        state.route = route;
        Ok(())
    }

    pub(crate) fn disable(&mut self, source: IrqSource) {
        self.source_state_mut(source).enabled = false;
    }

    pub(crate) fn mask(&mut self, source: IrqSource) {
        self.source_state_mut(source).masked = true;
    }

    pub(crate) fn unmask(&mut self, source: IrqSource) {
        self.source_state_mut(source).masked = false;
    }

    pub(crate) fn latch(&mut self, source: IrqSource) -> bool {
        let state = self.source_state_mut(source);
        if state.enabled {
            state.latch();
            return true;
        }
        false
    }

    pub(crate) fn ack(&mut self, source: IrqSource, records_pending: bool) {
        self.source_state_mut(source).ack(records_pending);
    }

    pub(crate) fn pending_snapshot(&self) -> Option<IrqSnapshot> {
        let pending = self
            .sources
            .values()
            .find(|state| state.has_line_cause())
            .copied()?;
        let asserted = self.sources.values().any(PdmaIrqSourceState::asserts_line);
        let state = self
            .sources
            .values()
            .find(|state| state.asserts_line())
            .copied()
            .unwrap_or(pending);
        let line = state.line()?;
        Some(IrqSnapshot::new(
            IrqCause::new(line, EpochIrqSource::new(self.epoch, state.source)),
            asserted,
        ))
    }

    pub(crate) fn take_notification(&mut self) -> Option<IrqNotification> {
        if let Some(state) = self.sources.values().find(|state| state.asserts_line()) {
            if let Some(line) = state.line() {
                return Some(IrqNotification::Line(IrqSnapshot::new(
                    IrqCause::new(line, EpochIrqSource::new(self.epoch, state.source)),
                    true,
                )));
            }
        }
        self.sources
            .values_mut()
            .find(|state| state.can_deliver_message())
            .and_then(|state| state.take_message(self.epoch))
            .map(IrqNotification::Message)
    }

    pub(crate) fn observe(&self, source: IrqSource) -> IrqSourceObservation {
        let state = self
            .sources
            .get(&source)
            .copied()
            .unwrap_or_else(|| PdmaIrqSourceState::new(source, self.line));
        IrqSourceObservation::new(
            state.route,
            state.pending_messages,
            state.observation_bits(),
        )
    }

    pub(crate) fn notification_ready(&self) -> bool {
        self.sources
            .values()
            .any(|state| state.asserts_line() || state.can_deliver_message())
    }

    pub(crate) fn line_asserted(&self) -> bool {
        self.sources.values().any(PdmaIrqSourceState::asserts_line)
    }

    fn source_state_mut(&mut self, source: IrqSource) -> &mut PdmaIrqSourceState {
        let line = self.line;
        self.sources
            .entry(source)
            .or_insert_with(|| PdmaIrqSourceState::new(source, line))
    }
}

impl PdmaIrqSourceState {
    const fn observation_bits(self) -> u8 {
        flag(self.enabled, 1)
            | flag(self.masked, 2)
            | flag(self.cause_pending, 4)
            | flag(self.asserts_line() || self.can_deliver_message(), 8)
            | flag(self.asserts_line(), 16)
    }
    const fn new(source: IrqSource, line: IrqLine) -> Self {
        Self {
            source,
            route: IrqRoute::Line(line),
            enabled: false,
            masked: true,
            cause_pending: false,
            pending_messages: 0,
            next_message_id: 0,
        }
    }

    const fn asserts_line(&self) -> bool {
        self.has_line_cause() && self.enabled && !self.masked
    }

    const fn has_line_cause(&self) -> bool {
        matches!(self.route, IrqRoute::Line(_)) && self.cause_pending
    }

    const fn line(&self) -> Option<IrqLine> {
        match self.route {
            IrqRoute::Line(line) => Some(line),
            IrqRoute::Message(_) => None,
        }
    }

    const fn is_active(&self) -> bool {
        self.enabled || self.cause_pending || self.pending_messages != 0
    }

    const fn latch(&mut self) {
        self.cause_pending = true;
        match self.route {
            IrqRoute::Line(_) => {}
            IrqRoute::Message(_) => {
                self.pending_messages = self.pending_messages.saturating_add(1);
            }
        }
    }

    const fn ack(&mut self, records_pending: bool) {
        self.cause_pending =
            self.cause_pending && records_pending && matches!(self.route, IrqRoute::Line(_));
    }

    const fn can_deliver_message(&self) -> bool {
        matches!(self.route, IrqRoute::Message(_))
            && self.enabled
            && !self.masked
            && self.pending_messages != 0
    }

    const fn take_message(&mut self, epoch: DeviceEpoch) -> Option<IrqMessage> {
        if !self.can_deliver_message() {
            return None;
        }
        let vector = match self.route {
            IrqRoute::Line(_) => return None,
            IrqRoute::Message(vector) => vector,
        };
        let message = IrqMessage::new(
            IrqMessageId::new(self.next_message_id),
            vector,
            EpochIrqSource::new(epoch, self.source),
        );
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.pending_messages -= 1;
        Some(message)
    }
}

const fn flag(value: bool, mask: u8) -> u8 {
    if value { mask } else { 0 }
}

#[cfg(test)]
mod tests;
