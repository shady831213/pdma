use std::cell::RefCell;
use std::sync::{Arc, LockResult, Mutex, MutexGuard, Weak};

use super::{StorageBackend, StorageError};

thread_local! {
    static ACTIVE_BACKENDS: RefCell<Vec<Weak<()>>> = const { RefCell::new(Vec::new()) };
}

/// Clone-shared, thread-safe storage.
///
/// This policy is `Send` and `Sync` when `T: Send`. Each call holds one mutex
/// for its full closure, and poisoned mutexes recover the retained value.
#[derive(Debug)]
pub struct MutexStorage<T> {
    inner: Arc<MutexInner<T>>,
}

#[derive(Debug)]
struct MutexInner<T> {
    value: Mutex<T>,
    identity: Arc<()>,
}

impl<T> MutexStorage<T> {
    /// Creates shared storage containing `value`.
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(MutexInner {
                value: Mutex::new(value),
                identity: Arc::new(()),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, T> {
        self.lock_checked()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn lock_checked(&self) -> LockResult<MutexGuard<'_, T>> {
        self.inner.value.lock()
    }

    pub(crate) fn is_active(&self) -> bool {
        let identity = Arc::downgrade(&self.inner.identity);
        ACTIVE_BACKENDS.with(|active| {
            active
                .borrow()
                .iter()
                .any(|candidate| candidate.ptr_eq(&identity))
        })
    }

    pub(crate) fn enter(&self) -> Result<impl Drop, StorageError> {
        let identity = Arc::downgrade(&self.inner.identity);
        ACTIVE_BACKENDS.with(|active| {
            let mut active = active.borrow_mut();
            if active.iter().any(|candidate| candidate.ptr_eq(&identity)) {
                return Err(StorageError::Reentrant);
            }
            active.push(identity);
            Ok(SharedEntry)
        })
    }
}

struct SharedEntry;

impl Drop for SharedEntry {
    fn drop(&mut self) {
        ACTIVE_BACKENDS.with(|active| {
            active.borrow_mut().pop();
        });
    }
}

impl<T> From<T> for MutexStorage<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> Clone for MutexStorage<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> StorageBackend<T> for MutexStorage<T> {
    fn inspect<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value T) -> R,
    ) -> Result<R, StorageError> {
        let _entry = self.enter()?;
        let value = self.lock();
        Ok(operation(&value))
    }

    fn mutate<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value mut T) -> R,
    ) -> Result<R, StorageError> {
        let _entry = self.enter()?;
        let mut value = self.lock();
        Ok(operation(&mut value))
    }
}
