use super::PdmaControlError;
use pdma_model_contract::abi::{AbiCommand, AbiRingEntry, DmaAddress, RingLayout};
use pdma_model_contract::semantic::{RingCursor, RingCursors};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runner) struct RingState<R: AbiRingEntry> {
    layout: RingLayout<R>,
    cursors: RingCursors,
}

impl<R: AbiRingEntry> RingState<R> {
    pub(in crate::runner) fn new(layout: RingLayout<R>) -> Result<Self, PdmaControlError> {
        let zero = RingCursor::new(0);
        Ok(Self {
            layout,
            cursors: RingCursors::try_new(layout.depth(), zero, zero)?,
        })
    }

    pub(in crate::runner) const fn layout(self) -> RingLayout<R> {
        self.layout
    }

    pub(in crate::runner) const fn cursors(self) -> RingCursors {
        self.cursors
    }

    pub(in crate::runner) const fn producer(self) -> RingCursor {
        self.cursors.producer()
    }

    pub(in crate::runner) const fn consumer(self) -> RingCursor {
        self.cursors.consumer()
    }

    pub(in crate::runner) fn is_full(self) -> bool {
        self.cursors.is_full()
    }

    pub(in crate::runner) fn update_producer(
        self,
        cursor: RingCursor,
    ) -> Result<Self, PdmaControlError> {
        Ok(Self {
            cursors: self.cursors.update_producer(cursor)?,
            ..self
        })
    }

    pub(in crate::runner) fn update_consumer(
        self,
        cursor: RingCursor,
    ) -> Result<Self, PdmaControlError> {
        Ok(Self {
            cursors: self.cursors.update_consumer(cursor)?,
            ..self
        })
    }

    pub(in crate::runner) fn entry_address(
        self,
        cursor: RingCursor,
    ) -> Result<DmaAddress, PdmaControlError> {
        self.layout
            .entry_address(cursor.raw())
            .map_err(|_| PdmaControlError::InvalidTransaction)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runner) struct SubmissionRingState {
    ring: RingState<AbiCommand>,
    captured_producer: RingCursor,
}

impl SubmissionRingState {
    pub(in crate::runner) fn new(layout: RingLayout<AbiCommand>) -> Result<Self, PdmaControlError> {
        Ok(Self {
            ring: RingState::new(layout)?,
            captured_producer: RingCursor::new(0),
        })
    }

    pub(in crate::runner) const fn ring(self) -> RingState<AbiCommand> {
        self.ring
    }

    pub(in crate::runner) const fn captured_producer(self) -> RingCursor {
        self.captured_producer
    }

    pub(in crate::runner) fn update_producer(
        self,
        cursor: RingCursor,
    ) -> Result<Self, PdmaControlError> {
        Ok(Self {
            ring: self.ring.update_producer(cursor)?,
            ..self
        })
    }

    pub(in crate::runner) fn update_consumer(
        self,
        cursor: RingCursor,
    ) -> Result<Self, PdmaControlError> {
        Ok(Self {
            ring: self.ring.update_consumer(cursor)?,
            ..self
        })
    }

    pub(in crate::runner) fn consume_captured(
        self,
        cursor: RingCursor,
    ) -> Result<Self, PdmaControlError> {
        if cursor > self.captured_producer {
            return Err(PdmaControlError::InvalidTransaction);
        }
        self.update_consumer(cursor)
    }

    pub(in crate::runner) fn capture(self, cursor: RingCursor) -> Result<Self, PdmaControlError> {
        if cursor < self.captured_producer || cursor > self.ring.producer() {
            return Err(PdmaControlError::InvalidTransaction);
        }
        Ok(Self {
            captured_producer: cursor,
            ..self
        })
    }

    pub(in crate::runner) fn has_captured_entry(self) -> bool {
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
            .expect("initial ring must be valid")
            .update_producer(RingCursor::new(1))
            .expect("producer publication must be valid");

        let updated = ring
            .update_consumer(RingCursor::new(1))
            .expect("software may discard an uncaptured submission");

        assert_eq!(updated.ring().consumer(), RingCursor::new(1));
        assert_eq!(updated.captured_producer(), RingCursor::new(0));
    }
}
