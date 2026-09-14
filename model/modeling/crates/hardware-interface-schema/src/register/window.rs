mod error;

pub use error::RegisterAccessError;

use crate::{Access, RegisterDefinition, RegisterMapLayout};

/// An addressed register map with one authoritative generated value.
///
/// Access hooks run synchronously and are never retained. Transactions stage a
/// temporary value; validation and rejected `before` hooks preserve stored values.
/// Mutations of external captures are not rolled back: a rejecting `before` hook
/// should avoid external effects.
#[derive(Debug)]
pub struct RegisterWindow<M: RegisterMapLayout> {
    base: usize,
    /// Hardware-owned values; direct access bypasses software permissions.
    pub values: M::Value,
}

impl<M: RegisterMapLayout> RegisterWindow<M> {
    /// Creates a window at `base` with the generated register defaults.
    pub const fn new(base: usize) -> Self {
        Self {
            base,
            values: M::DEFAULT,
        }
    }

    /// Returns the address of the window's first byte.
    pub const fn base(&self) -> usize {
        self.base
    }

    /// Reads one exact leaf without hardware hooks.
    pub fn read(&mut self, address: usize, out: &mut [u8]) -> Result<(), RegisterAccessError> {
        self.read_with(address, out, |_, _| Ok(()), |_, _| {})
    }

    /// Writes one exact leaf without hardware hooks.
    pub fn write(&mut self, address: usize, source: &[u8]) -> Result<(), RegisterAccessError> {
        self.write_with(address, source, |_, _, _| Ok(()), |_, _| {})
    }

    /// Refreshes a staged value before reading, then applies an infallible effect.
    ///
    /// Both hooks receive the exact window-relative leaf offset. Basic validation
    /// precedes all hooks. Bytes reflect the value before `after`, allowing read-clear
    /// behavior. Failed validation or `before` leaves values and output unchanged.
    pub fn read_with<E>(
        &mut self,
        address: usize,
        out: &mut [u8],
        before: impl FnOnce(usize, &mut M::Value) -> Result<(), E>,
        after: impl FnOnce(usize, &mut M::Value),
    ) -> Result<(), RegisterAccessError<E>> {
        let (offset, definition) = self
            .leaf(address, out.len())
            .map_err(RegisterAccessError::widen)?;
        if definition.access() == Access::WriteOnly {
            return Err(RegisterAccessError::ReadForbidden { offset });
        }
        let mut scratch = [0; 8];
        let bytes = scratch
            .get_mut(..out.len())
            .ok_or(RegisterAccessError::InvalidLayout { offset })?;
        let mut view = definition
            .view_mut(bytes)
            .map_err(|_| RegisterAccessError::InvalidLayout { offset })?;
        let mut next = self.values;
        before(offset, &mut next).map_err(RegisterAccessError::Other)?;
        let raw =
            M::read_register(&next, offset).ok_or(RegisterAccessError::InvalidLayout { offset })?;
        view.set_value(raw)
            .map_err(|_| RegisterAccessError::InvalidLayout { offset })?;
        after(offset, &mut next);
        self.values = next;
        out.copy_from_slice(bytes);
        Ok(())
    }

    /// Validates the current and proposed values before committing a write.
    ///
    /// Both hooks receive the exact window-relative leaf offset. Permissions,
    /// width and reserved bits are checked before hooks. `before` cannot mutate
    /// either snapshot; after its acceptance, `after` operates on committed values.
    pub fn write_with<E>(
        &mut self,
        address: usize,
        source: &[u8],
        before: impl FnOnce(usize, &M::Value, &M::Value) -> Result<(), E>,
        after: impl FnOnce(usize, &mut M::Value),
    ) -> Result<(), RegisterAccessError<E>> {
        let (offset, definition) = self
            .leaf(address, source.len())
            .map_err(RegisterAccessError::widen)?;
        if definition.access() == Access::ReadOnly {
            return Err(RegisterAccessError::WriteForbidden { offset });
        }
        let raw = definition
            .view(source)
            .and_then(|view| view.value())
            .map_err(|_| RegisterAccessError::InvalidLayout { offset })?;
        let old = M::read_register(&self.values, offset)
            .ok_or(RegisterAccessError::InvalidLayout { offset })?;
        let changed = (raw ^ old) & !definition.writable_mask();
        if changed != 0 {
            return Err(RegisterAccessError::ReservedBitChange {
                offset,
                writable_mask: definition.writable_mask(),
                changed,
            });
        }
        let mut next = self.values;
        M::write_register(&mut next, offset, raw)
            .ok_or(RegisterAccessError::InvalidLayout { offset })?;
        before(offset, &self.values, &next).map_err(RegisterAccessError::Other)?;
        self.values = next;
        after(offset, &mut self.values);
        Ok(())
    }

    fn leaf(
        &self,
        address: usize,
        width: usize,
    ) -> Result<(usize, RegisterDefinition<u16>), RegisterAccessError> {
        let offset = address
            .checked_sub(self.base)
            .filter(|offset| *offset < M::EXTENT)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        let definition =
            M::register_definition(offset).ok_or(RegisterAccessError::InvalidOffset { offset })?;
        if definition.width() != width {
            return Err(RegisterAccessError::WidthMismatch {
                offset,
                expected: definition.width(),
                actual: width,
            });
        }
        if offset.checked_add(width).is_none_or(|end| end > M::EXTENT)
            || width
                .checked_sub(1)
                .and_then(|last| address.checked_add(last))
                .is_none()
        {
            return Err(RegisterAccessError::InvalidLayout { offset });
        }
        Ok((offset, definition))
    }
}
