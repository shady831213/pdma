//! Generic single-producer/single-consumer ring model with wrap behavior.

use crate::{ModelError, ModelResult};

/// A fixed-capacity ring that tracks producer and consumer wrap naturally.
#[derive(Debug, Clone)]
pub struct Ring<T> {
    entries: Vec<Option<T>>,
    head: usize,
    tail: usize,
    len: usize,
}

impl<T> Ring<T> {
    /// Creates a ring with checked non-zero capacity.
    pub fn with_capacity(capacity: usize) -> ModelResult<Self> {
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

    /// Returns the fixed ring capacity.
    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// Returns the number of occupied entries.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true when no entries are occupied.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns true when every entry is occupied.
    pub fn is_full(&self) -> bool {
        self.len == self.capacity()
    }

    /// Pushes one item or reports a full ring.
    pub fn push(&mut self, item: T) -> ModelResult<()> {
        if self.is_full() {
            return Err(ModelError::RingFull);
        }
        if let Some(slot) = self.entries.get_mut(self.tail) {
            *slot = Some(item);
        }
        self.tail = self.wrap_next(self.tail);
        self.len += 1;
        Ok(())
    }

    /// Pops one item, returning `None` when the ring is empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let item = self.entries.get_mut(self.head).and_then(Option::take);
        self.head = self.wrap_next(self.head);
        self.len -= 1;
        item
    }

    /// Returns the head item without consuming it.
    pub fn front(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }
        self.entries.get(self.head).and_then(Option::as_ref)
    }

    fn wrap_next(&self, value: usize) -> usize {
        let next = value + 1;
        if next == self.capacity() { 0 } else { next }
    }
}

#[cfg(test)]
mod tests {
    use super::Ring;
    use crate::ModelError;

    #[test]
    fn reports_full_empty_and_wrap_when_capacity_is_reused() {
        // Given: a two-entry ring.
        let mut ring = Ring::with_capacity(2).expect("capacity is valid");

        // When: entries are pushed, popped, and pushed across the wrap point.
        assert!(ring.is_empty());
        ring.push(1).expect("first push has space");
        ring.push(2).expect("second push has space");

        // Then: full push is rejected and FIFO order survives wrap-around reuse.
        assert_eq!(ring.push(3), Err(ModelError::RingFull));
        assert_eq!(ring.pop(), Some(1));
        ring.push(3).expect("wrapped slot is reusable");
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), None);
    }
}
