use pdma_model_contract::{IrqCause, IrqLine, IrqSnapshot, IrqSource};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct PdmaIrqController {
    line: IrqLine,
    sources: BTreeMap<IrqSource, PdmaIrqSourceState>,
}

#[derive(Debug, Clone, Copy)]
struct PdmaIrqSourceState {
    source: IrqSource,
    enabled: bool,
    masked: bool,
    pending: bool,
}

impl PdmaIrqController {
    pub(super) const fn new(line: IrqLine) -> Self {
        Self {
            line,
            sources: BTreeMap::new(),
        }
    }

    pub(super) fn enable(&mut self, source: IrqSource) {
        self.source_state_mut(source).enabled = true;
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

    pub(super) fn latch(&mut self, source: IrqSource) {
        let state = self.source_state_mut(source);
        if state.enabled {
            state.pending = true;
        }
    }

    pub(super) fn ack(&mut self, source: IrqSource) {
        self.source_state_mut(source).pending = false;
    }

    pub(super) fn pending_snapshot(&self) -> Option<IrqSnapshot> {
        let pending = self.sources.values().find(|state| state.pending).copied()?;
        let asserted = self.sources.values().any(PdmaIrqSourceState::asserts_line);
        let state = self
            .sources
            .values()
            .find(|state| state.asserts_line())
            .copied()
            .unwrap_or(pending);
        Some(IrqSnapshot::new(
            IrqCause::new(self.line, state.source),
            asserted,
        ))
    }

    fn source_state_mut(&mut self, source: IrqSource) -> &mut PdmaIrqSourceState {
        self.sources
            .entry(source)
            .or_insert_with(|| PdmaIrqSourceState::new(source))
    }
}

impl PdmaIrqSourceState {
    const fn new(source: IrqSource) -> Self {
        Self {
            source,
            enabled: false,
            masked: true,
            pending: false,
        }
    }

    const fn asserts_line(&self) -> bool {
        self.pending && self.enabled && !self.masked
    }
}

#[cfg(test)]
mod tests {
    use super::PdmaIrqController;
    use pdma_model_contract::{CompletionRingId, IrqLine, IrqSource, IrqVector};

    #[test]
    fn latch_asserts_enabled_unmasked_source() {
        // Given: an enabled and unmasked completion IRQ source.
        let line = IrqLine::new(IrqVector::new(0));
        let source = IrqSource::Completion(CompletionRingId::new(1));
        let mut controller = PdmaIrqController::new(line);
        controller.enable(source);
        controller.unmask(source);

        // When: the device latches the source.
        controller.latch(source);

        // Then: software can observe an asserted pending IRQ for the source.
        let snapshot = controller
            .pending_snapshot()
            .expect("enabled source should latch pending IRQ");
        assert!(snapshot.asserted());
        assert_eq!(snapshot.cause().source(), source);
        assert_eq!(snapshot.cause().line(), line);
    }

    #[test]
    fn masked_source_latches_without_asserting_line() {
        // Given: an enabled but masked completion IRQ source.
        let line = IrqLine::new(IrqVector::new(0));
        let source = IrqSource::Completion(CompletionRingId::new(2));
        let mut controller = PdmaIrqController::new(line);
        controller.enable(source);

        // When: the device latches the source.
        controller.latch(source);

        // Then: pending cause is preserved while line assertion is suppressed.
        let snapshot = controller
            .pending_snapshot()
            .expect("masked source should remain pending");
        assert!(!snapshot.asserted());
        assert_eq!(snapshot.cause().source(), source);
    }

    #[test]
    fn ack_clears_pending_without_changing_mask_state() {
        // Given: a masked pending IRQ source.
        let line = IrqLine::new(IrqVector::new(0));
        let source = IrqSource::Completion(CompletionRingId::new(3));
        let mut controller = PdmaIrqController::new(line);
        controller.enable(source);
        controller.latch(source);

        // When: software acknowledges the source and the device latches it again.
        controller.ack(source);
        controller.latch(source);

        // Then: the source remains masked after ack and still does not assert.
        let snapshot = controller
            .pending_snapshot()
            .expect("source should latch again after ack");
        assert!(!snapshot.asserted());
    }

    #[test]
    fn disabled_source_does_not_latch() {
        // Given: a disabled IRQ source.
        let source = IrqSource::Completion(CompletionRingId::new(4));
        let mut controller = PdmaIrqController::new(IrqLine::new(IrqVector::new(0)));

        // When: the device attempts to latch the disabled source.
        controller.latch(source);

        // Then: no pending IRQ is recorded.
        assert_eq!(controller.pending_snapshot(), None);
    }

    #[test]
    fn disabled_pending_source_stays_pending_without_asserting_line() {
        // Given: an enabled pending source whose line is asserted.
        let source = IrqSource::Completion(CompletionRingId::new(5));
        let mut controller = PdmaIrqController::new(IrqLine::new(IrqVector::new(0)));
        controller.enable(source);
        controller.unmask(source);
        controller.latch(source);

        // When: software disables that source before acking it.
        controller.disable(source);

        // Then: pending cause remains visible but delivery is deasserted.
        let snapshot = controller
            .pending_snapshot()
            .expect("disabled pending source should stay visible");
        assert!(!snapshot.asserted());
    }

    #[test]
    fn remasking_pending_source_deasserts_line() {
        // Given: an asserted pending source.
        let source = IrqSource::Completion(CompletionRingId::new(6));
        let mut controller = PdmaIrqController::new(IrqLine::new(IrqVector::new(0)));
        controller.enable(source);
        controller.unmask(source);
        controller.latch(source);

        // When: software masks that source.
        controller.mask(source);

        // Then: pending cause remains visible but delivery is suppressed.
        let snapshot = controller
            .pending_snapshot()
            .expect("masked pending source should stay visible");
        assert!(!snapshot.asserted());
    }

    #[test]
    fn asserted_source_takes_priority_over_masked_pending_source() {
        // Given: a masked lower source and an unmasked higher source are both pending.
        let masked = IrqSource::Completion(CompletionRingId::new(0));
        let asserted = IrqSource::Completion(CompletionRingId::new(1));
        let mut controller = PdmaIrqController::new(IrqLine::new(IrqVector::new(0)));
        controller.enable(masked);
        controller.latch(masked);
        controller.enable(asserted);
        controller.unmask(asserted);
        controller.latch(asserted);

        // When: software observes the pending IRQ snapshot.
        let snapshot = controller
            .pending_snapshot()
            .expect("pending sources should produce a snapshot");

        // Then: the asserted source is selected instead of the masked lower source.
        assert!(snapshot.asserted());
        assert_eq!(snapshot.cause().source(), asserted);
    }
}
