//! Internal exact-ABI ring cursor state.

use crate::abi::ring::PdmaControlError;
use pdma_model_contract::abi::{
    AbiCommand, RingCursor as AbiRingCursor, RingLayout, RingState as AbiRingState,
};
use pdma_model_contract::semantic::{RingCursor as SemanticRingCursor, RingCursors};

pub(crate) type RingState<R> = AbiRingState<R>;

pub(crate) const fn exact_cursor(cursor: SemanticRingCursor) -> AbiRingCursor {
    AbiRingCursor::new(cursor.raw())
}

pub(super) fn semantic_cursors<R: pdma_model_contract::abi::AbiRingEntry>(
    state: AbiRingState<R>,
) -> Result<RingCursors, PdmaControlError> {
    RingCursors::try_new(
        state.layout().depth(),
        SemanticRingCursor::new(state.producer().raw()),
        SemanticRingCursor::new(state.consumer().raw()),
    )
    .map_err(PdmaControlError::from)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SubmissionRingState {
    ring: AbiRingState<AbiCommand>,
    captured_producer: AbiRingCursor,
}

impl SubmissionRingState {
    pub(crate) const fn new(layout: RingLayout<AbiCommand>) -> Self {
        Self {
            ring: AbiRingState::empty(layout),
            captured_producer: AbiRingCursor::new(0),
        }
    }

    pub(crate) const fn ring(self) -> AbiRingState<AbiCommand> {
        self.ring
    }

    pub(crate) const fn captured_producer(self) -> AbiRingCursor {
        self.captured_producer
    }

    pub(crate) fn update_producer(self, cursor: AbiRingCursor) -> Result<Self, PdmaControlError> {
        Ok(Self {
            ring: self.ring.update_producer(cursor)?,
            ..self
        })
    }

    pub(crate) fn update_consumer(self, cursor: AbiRingCursor) -> Result<Self, PdmaControlError> {
        Ok(Self {
            ring: self.ring.update_consumer(cursor)?,
            ..self
        })
    }

    pub(crate) fn consume_captured(self, cursor: AbiRingCursor) -> Result<Self, PdmaControlError> {
        if cursor > self.captured_producer {
            return Err(PdmaControlError::InvalidTransaction);
        }
        self.update_consumer(cursor)
    }

    pub(crate) fn capture(self, cursor: AbiRingCursor) -> Result<Self, PdmaControlError> {
        if cursor < self.captured_producer || cursor > self.ring.producer() {
            return Err(PdmaControlError::InvalidTransaction);
        }
        Ok(Self {
            captured_producer: cursor,
            ..self
        })
    }

    pub(crate) fn has_captured_entry(self) -> bool {
        self.ring.consumer() < self.captured_producer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdma_model_contract::abi::{DmaAddress, DmaRegion};

    #[test]
    fn explicit_consumer_can_discard_an_uncaptured_submission() {
        let region =
            DmaRegion::try_new(DmaAddress::new(0x1000), 128).expect("test region must be valid");
        let layout = RingLayout::try_new(region, 2).expect("test layout must be valid");
        let ring = SubmissionRingState::new(layout)
            .update_producer(AbiRingCursor::new(1))
            .expect("producer publication must be valid");

        let updated = ring
            .update_consumer(AbiRingCursor::new(1))
            .expect("software may discard an uncaptured submission");

        assert_eq!(updated.ring().consumer(), AbiRingCursor::new(1));
        assert_eq!(updated.captured_producer(), AbiRingCursor::new(0));
    }
}
