use std::cell::{Cell, RefCell};

use hardware_interface_runtime::{StorageBackend, StorageError};

#[cfg(test)]
thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static LIVE_IMAGES: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn allocation_observation() -> (usize, usize) {
    (ALLOCATIONS.get(), LIVE_IMAGES.get())
}

#[derive(Debug)]
pub(super) struct DeviceRegisterStorage {
    value: RefCell<Vec<u8>>,
    active: Cell<bool>,
}

impl DeviceRegisterStorage {
    fn enter(&self) -> Result<DeviceRegisterEntry<'_>, StorageError> {
        if self.active.replace(true) {
            return Err(StorageError::Reentrant);
        }
        Ok(DeviceRegisterEntry(&self.active))
    }
}

impl From<Vec<u8>> for DeviceRegisterStorage {
    fn from(value: Vec<u8>) -> Self {
        #[cfg(test)]
        ALLOCATIONS.set(ALLOCATIONS.get().saturating_add(1));
        #[cfg(test)]
        LIVE_IMAGES.set(LIVE_IMAGES.get().saturating_add(1));
        Self {
            value: RefCell::new(value),
            active: Cell::new(false),
        }
    }
}

#[cfg(test)]
impl Drop for DeviceRegisterStorage {
    fn drop(&mut self) {
        #[cfg(test)]
        LIVE_IMAGES.set(LIVE_IMAGES.get().saturating_sub(1));
    }
}

impl StorageBackend<Vec<u8>> for DeviceRegisterStorage {
    fn inspect<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value Vec<u8>) -> R,
    ) -> Result<R, StorageError> {
        let _entry = self.enter()?;
        let value = self
            .value
            .try_borrow()
            .map_err(|_| StorageError::Reentrant)?;
        Ok(operation(&value))
    }

    fn mutate<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value mut Vec<u8>) -> R,
    ) -> Result<R, StorageError> {
        let _entry = self.enter()?;
        let mut value = self
            .value
            .try_borrow_mut()
            .map_err(|_| StorageError::Reentrant)?;
        Ok(operation(&mut value))
    }
}

struct DeviceRegisterEntry<'active>(&'active Cell<bool>);

impl Drop for DeviceRegisterEntry<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}

    #[test]
    fn device_storage_is_move_safe_and_rejects_reentry() {
        assert_send::<DeviceRegisterStorage>();
        let storage = DeviceRegisterStorage::from(vec![0]);
        let nested =
            storage.inspect(|_| storage.mutate(|bytes| bytes.first_mut().map(|byte| *byte = 1)));
        assert_eq!(nested, Ok(Err(StorageError::Reentrant)));
        assert_eq!(storage.inspect(|bytes| bytes.first().copied()), Ok(Some(0)));
    }
}
