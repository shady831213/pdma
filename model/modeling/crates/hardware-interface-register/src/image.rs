use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;

use hardware_interface_schema::{
    Access, RegisterDefinition, RegisterLayout, RegisterMapLayout, SchemaCodecError,
};

use super::{RegisterBusOffset, RegisterBusWidth, RegisterEndpointError};
use hardware_interface_storage::{StorageBackend, StorageError};

/// Failure while constructing or viewing one register image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterImageError {
    /// Generated layout encoding or view construction failed.
    Codec(SchemaCodecError),
    /// The selected storage policy rejected the operation.
    Storage(StorageError),
}

impl Display for RegisterImageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(source) => Display::fmt(source, formatter),
            Self::Storage(source) => Display::fmt(source, formatter),
        }
    }
}

impl Error for RegisterImageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Codec(source) => Some(source),
            Self::Storage(source) => Some(source),
        }
    }
}

/// One authoritative encoded register-layout image behind a storage policy.
///
/// Typed views cannot escape their storage call:
///
/// ```compile_fail
/// use hardware_interface_register::RegisterImage;
/// use hardware_interface_storage::StorageBackend;
/// use hardware_interface_schema::RegisterLayout;
/// fn escape<M: RegisterLayout, B: StorageBackend<Vec<u8>>>(image: &RegisterImage<M, B>) {
///     let _escaped = image.view(|view| view);
/// }
/// ```
///
/// Futures borrowing mutable views cannot escape:
///
/// ```compile_fail
/// use hardware_interface_register::RegisterImage;
/// use hardware_interface_storage::StorageBackend;
/// use hardware_interface_schema::RegisterLayout;
/// fn escape<M: RegisterLayout, B: StorageBackend<Vec<u8>>>(image: &RegisterImage<M, B>) {
///     let _future = image.view_mut(|view| async move { drop(view) });
/// }
/// ```
///
/// Raw bytes and backend guards have no public access surface:
///
/// ```compile_fail
/// use hardware_interface_register::RegisterImage;
/// use hardware_interface_storage::StorageBackend;
/// use hardware_interface_schema::RegisterLayout;
/// fn expose<M: RegisterLayout, B: StorageBackend<Vec<u8>>>(image: &RegisterImage<M, B>) {
///     let _bytes = image.bytes();
///     let _guard = image.backend();
/// }
/// ```
#[derive(Debug)]
pub struct RegisterImage<M: RegisterLayout, B: StorageBackend<Vec<u8>>> {
    backend: B,
    marker: PhantomData<fn() -> M>,
}

impl<M, B> RegisterImage<M, B>
where
    M: RegisterLayout,
    B: StorageBackend<Vec<u8>> + From<Vec<u8>>,
{
    /// Encodes the generated default into one exact-size root byte vector.
    pub fn new() -> Result<Self, RegisterImageError> {
        let mut bytes = vec![0_u8; M::EXTENT];
        M::encode(&mut bytes, &M::DEFAULT).map_err(RegisterImageError::Codec)?;
        Ok(Self {
            backend: B::from(bytes),
            marker: PhantomData,
        })
    }
}

impl<M, B> RegisterImage<M, B>
where
    M: RegisterMapLayout,
    B: StorageBackend<Vec<u8>>,
{
    /// Reads one exact-start leaf without changing `destination` on failure.
    pub fn read_leaf(
        &self,
        offset: RegisterBusOffset,
        destination: &mut [u8],
    ) -> Result<(), RegisterEndpointError> {
        let width = RegisterBusWidth::new(destination.len());
        let definition = validate_leaf::<M>(offset, width)?;
        if definition.access() == Access::WriteOnly {
            return Err(RegisterEndpointError::ReadForbidden { offset });
        }
        let prepared = self
            .backend
            .inspect(|bytes| leaf(bytes, offset, definition).map(<[u8]>::to_vec))
            .map_err(RegisterEndpointError::Storage)??;
        destination.copy_from_slice(&prepared);
        Ok(())
    }

    /// Writes one exact-start leaf after complete scratch validation.
    pub fn write_leaf(
        &self,
        offset: RegisterBusOffset,
        source: &[u8],
    ) -> Result<(), RegisterEndpointError> {
        let width = RegisterBusWidth::new(source.len());
        let definition = validate_leaf::<M>(offset, width)?;
        if definition.access() == Access::ReadOnly {
            return Err(RegisterEndpointError::WriteForbidden { offset });
        }
        self.backend
            .mutate(|bytes| prepare_and_commit(bytes, offset, definition, source))
            .map_err(RegisterEndpointError::Storage)?
    }
}

impl<M, B> RegisterImage<M, B>
where
    M: RegisterLayout,
    B: StorageBackend<Vec<u8>>,
{
    /// Runs one immutable generated view without exposing bytes or storage guards.
    pub fn view<R>(
        &self,
        operation: impl for<'view> FnOnce(M::View<'view>) -> R,
    ) -> Result<R, RegisterImageError> {
        self.backend
            .inspect(|bytes| M::view(bytes).map(operation))
            .map_err(RegisterImageError::Storage)?
            .map_err(RegisterImageError::Codec)
    }

    /// Runs one mutable generated view and commits only after normal return.
    pub fn view_mut<R>(
        &self,
        operation: impl for<'view> FnOnce(M::ViewMut<'view>) -> R,
    ) -> Result<R, RegisterImageError> {
        self.backend
            .mutate(|bytes| {
                let mut prepared = Vec::with_capacity(bytes.len());
                prepared.extend_from_slice(bytes);
                let result = M::view_mut(&mut prepared).map(operation)?;
                bytes.copy_from_slice(&prepared);
                Ok(result)
            })
            .map_err(RegisterImageError::Storage)?
            .map_err(RegisterImageError::Codec)
    }
}

impl<M, B> Clone for RegisterImage<M, B>
where
    M: RegisterLayout,
    B: StorageBackend<Vec<u8>> + Clone,
{
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            marker: PhantomData,
        }
    }
}

fn validate_leaf<M: RegisterMapLayout>(
    offset: RegisterBusOffset,
    width: RegisterBusWidth,
) -> Result<RegisterDefinition<u16>, RegisterEndpointError> {
    let start = usize::try_from(offset.raw())
        .map_err(|_| RegisterEndpointError::UnmappedOffset { offset })?;
    let definition =
        M::register_definition(start).ok_or(RegisterEndpointError::UnmappedOffset { offset })?;
    if definition.width() != width.raw() {
        return Err(RegisterEndpointError::WidthMismatch {
            offset,
            expected: definition.width(),
            actual: width.raw(),
        });
    }
    Ok(definition)
}

fn prepare_and_commit(
    bytes: &mut [u8],
    offset: RegisterBusOffset,
    definition: RegisterDefinition<u16>,
    source: &[u8],
) -> Result<(), RegisterEndpointError> {
    let current = leaf(bytes, offset, definition)?;
    let mut prepared = [0_u8; 8];
    let prepared_value = prepared
        .get_mut(..definition.width())
        .ok_or(RegisterEndpointError::InvalidLayout { offset })?;
    prepared_value.copy_from_slice(source);
    let old = definition
        .view(current)
        .and_then(|view| view.value())
        .map_err(|_| RegisterEndpointError::InvalidLayout { offset })?;
    let new = definition
        .view(prepared_value)
        .and_then(|view| view.value())
        .map_err(|_| RegisterEndpointError::InvalidLayout { offset })?;
    let changed = (old ^ new) & !definition.writable_mask();
    if changed != 0 {
        return Err(RegisterEndpointError::ReservedBitChange {
            offset,
            writable_mask: definition.writable_mask(),
            changed,
        });
    }
    leaf_mut(bytes, offset, definition)?.copy_from_slice(prepared_value);
    Ok(())
}

fn leaf(
    bytes: &[u8],
    offset: RegisterBusOffset,
    definition: RegisterDefinition<u16>,
) -> Result<&[u8], RegisterEndpointError> {
    let start = usize::try_from(offset.raw())
        .map_err(|_| RegisterEndpointError::InvalidLayout { offset })?;
    let end = start
        .checked_add(definition.width())
        .ok_or(RegisterEndpointError::InvalidLayout { offset })?;
    bytes
        .get(start..end)
        .ok_or(RegisterEndpointError::InvalidLayout { offset })
}

fn leaf_mut(
    bytes: &mut [u8],
    offset: RegisterBusOffset,
    definition: RegisterDefinition<u16>,
) -> Result<&mut [u8], RegisterEndpointError> {
    let start = usize::try_from(offset.raw())
        .map_err(|_| RegisterEndpointError::InvalidLayout { offset })?;
    let end = start
        .checked_add(definition.width())
        .ok_or(RegisterEndpointError::InvalidLayout { offset })?;
    bytes
        .get_mut(start..end)
        .ok_or(RegisterEndpointError::InvalidLayout { offset })
}
