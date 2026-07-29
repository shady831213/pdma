use pdma_model_contract::{
    DeviceEpoch, EpochIrqSource, IrqCause, IrqLine, IrqMessage, IrqMessageId, IrqNotification,
    IrqRoute, IrqSnapshot, IrqSource, ModelError, ModelResult,
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct PdmaIrqController {
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
    pub(super) const fn new(epoch: DeviceEpoch, line: IrqLine) -> Self {
        Self {
            epoch,
            line,
            sources: BTreeMap::new(),
        }
    }

    pub(super) fn enable(&mut self, source: IrqSource) {
        self.source_state_mut(source).enabled = true;
    }

    pub(super) fn configure_route(
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

    pub(super) fn disable(&mut self, source: IrqSource) {
        self.source_state_mut(source).enabled = false;
    }

    pub(super) fn mask(&mut self, source: IrqSource) {
        self.source_state_mut(source).masked = true;
    }

    pub(super) fn unmask(&mut self, source: IrqSource) {
        self.source_state_mut(source).masked = false;
    }

    pub(super) fn latch(&mut self, source: IrqSource) -> bool {
        let state = self.source_state_mut(source);
        if state.enabled {
            state.latch();
            return true;
        }
        false
    }

    pub(super) fn ack(&mut self, source: IrqSource, records_pending: bool) {
        self.source_state_mut(source).ack(records_pending);
    }

    pub(super) fn pending_snapshot(&self) -> Option<IrqSnapshot> {
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

    pub(super) fn take_notification(&mut self) -> Option<IrqNotification> {
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

    pub(super) fn line_asserted(&self) -> bool {
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

#[cfg(test)]
mod tests;
