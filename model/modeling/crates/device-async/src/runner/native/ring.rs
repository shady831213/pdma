use pdma_model_contract::error::{ModelError, ModelResult};

#[derive(Debug)]
pub(super) struct NativeRing<T> {
    entries: Vec<Option<T>>,
    head: usize,
    tail: usize,
    len: usize,
}

impl<T> NativeRing<T> {
    pub(super) fn with_capacity(capacity: usize) -> ModelResult<Self> {
        if capacity == 0 {
            return Err(ModelError::ZeroCapacity);
        }
        let entries = (0..capacity).map(|_| None).collect();
        Ok(Self {
            entries,
            head: 0,
            tail: 0,
            len: 0,
        })
    }

    pub(super) fn is_full(&self) -> bool {
        self.len == self.entries.len()
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(super) fn push(&mut self, item: T) -> ModelResult<()> {
        if self.is_full() {
            return Err(ModelError::RingFull);
        }
        if let Some(slot) = self.entries.get_mut(self.tail) {
            *slot = Some(item);
        }
        self.tail = self.wrap_next(self.tail);
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    pub(super) fn front(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }
        self.entries.get(self.head).and_then(Option::as_ref)
    }

    pub(super) fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let item = self.entries.get_mut(self.head).and_then(Option::take);
        self.head = self.wrap_next(self.head);
        self.len -= 1;
        item
    }

    fn wrap_next(&self, value: usize) -> usize {
        let next = value.saturating_add(1);
        if next == self.entries.len() { 0 } else { next }
    }
}
