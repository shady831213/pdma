use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{StorageBackend, StorageError};

/// Clone-shared, single-thread storage.
///
/// This policy is neither `Send` nor `Sync`. Clones share one value and one
/// reentry state. Every call completes synchronously before another may begin.
#[derive(Debug)]
pub struct LocalStorage<T> {
    inner: Rc<LocalInner<T>>,
}

#[derive(Debug)]
struct LocalInner<T> {
    value: RefCell<T>,
    active: Cell<bool>,
}

impl<T> LocalStorage<T> {
    /// Creates local storage containing `value`.
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(LocalInner {
                value: RefCell::new(value),
                active: Cell::new(false),
            }),
        }
    }

    fn enter(&self) -> Result<LocalEntry<'_>, StorageError> {
        if self.inner.active.replace(true) {
            return Err(StorageError::Reentrant);
        }
        Ok(LocalEntry(&self.inner.active))
    }
}

impl<T> Clone for LocalStorage<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T> StorageBackend<T> for LocalStorage<T> {
    fn inspect<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value T) -> R,
    ) -> Result<R, StorageError> {
        let _entry = self.enter()?;
        let value = self
            .inner
            .value
            .try_borrow()
            .map_err(|_| StorageError::Reentrant)?;
        Ok(operation(&value))
    }

    fn mutate<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value mut T) -> R,
    ) -> Result<R, StorageError> {
        let _entry = self.enter()?;
        let mut value = self
            .inner
            .value
            .try_borrow_mut()
            .map_err(|_| StorageError::Reentrant)?;
        Ok(operation(&mut value))
    }
}

struct LocalEntry<'active>(&'active Cell<bool>);

impl Drop for LocalEntry<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
