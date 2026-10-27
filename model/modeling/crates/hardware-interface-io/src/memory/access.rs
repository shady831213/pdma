use super::{MemoryAddress, MemoryAlignment};

mod unit {
    pub trait UnitSideband: Sized {
        fn unit() -> Self;
    }

    impl UnitSideband for () {
        fn unit() -> Self {}
    }
}

/// Synchronous checked bulk access to a modeled byte store.
///
/// Consumers depend on this capability rather than on backing ownership or an
/// allocator. `S` is opaque, caller-selected sideband; implementations interpret
/// it as needed. Only `S = ()` gets the convenience methods without sideband.
/// [`crate::memory::BoundMemory`] binds an explicit sideband to expose that simpler surface.
///
/// Ordinary reads and writes transfer bytes without encoding Rust values. The
/// buffer helpers preserve this rule; choose scalar byte order or a schema codec
/// in the consumer. Atomic operations have a separate, explicitly little-endian
/// word contract in [`crate::memory::AtomicOperation`].
///
/// # Implementing a backend
///
/// Supply validation, read, and write; override atomics only when supported.
/// Errors belong to the backend and need not implement the standard error trait.
/// Define ordinary access failure effects in that backend's contract; generic
/// helpers propagate errors without providing rollback. Validation is not a
/// reservation: callers cannot infer that a later access will still succeed.
pub trait MemoryAccess<S = ()> {
    /// Backend-owned failure, preserved by the default access helpers.
    type Error;

    /// Performs one indivisible 32-bit access, or rejects the unsupported capability.
    /// Implementations must honor [`crate::memory::AtomicOperation`]'s failure and ordering contract.
    fn atomic32_with_sideband(
        &self,
        _sideband: &S,
        _address: MemoryAddress,
        _operation: crate::memory::AtomicOperation<u32>,
    ) -> Result<u32, crate::memory::AtomicAccessError<Self::Error>> {
        Err(crate::memory::AtomicAccessError::Unsupported)
    }

    /// Performs one indivisible 64-bit access; errors must precede modification.
    fn atomic64_with_sideband(
        &self,
        _sideband: &S,
        _address: MemoryAddress,
        _operation: crate::memory::AtomicOperation<u64>,
    ) -> Result<u64, crate::memory::AtomicAccessError<Self::Error>> {
        Err(crate::memory::AtomicAccessError::Unsupported)
    }

    /// Performs an indivisible word access without metadata on a unit-sideband backend.
    fn atomic32(
        &self,
        address: MemoryAddress,
        operation: crate::memory::AtomicOperation<u32>,
    ) -> Result<u32, crate::memory::AtomicAccessError<Self::Error>>
    where
        S: unit::UnitSideband,
    {
        self.atomic32_with_sideband(&S::unit(), address, operation)
    }

    /// Performs an indivisible double-word access on a unit-sideband backend.
    fn atomic64(
        &self,
        address: MemoryAddress,
        operation: crate::memory::AtomicOperation<u64>,
    ) -> Result<u64, crate::memory::AtomicAccessError<Self::Error>>
    where
        S: unit::UnitSideband,
    {
        self.atomic64_with_sideband(&S::unit(), address, operation)
    }

    /// Checks a complete extent without reading, writing, allocating, or changing storage.
    ///
    /// The backend defines accepted extents and validation errors.
    ///
    /// # Errors
    /// Returns the backend error for a rejected extent or sideband.
    fn validate_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error>;

    /// Reads one contiguous slice into caller-owned storage.
    ///
    /// # Errors
    /// Returns the backend's validation or read error. Destination preservation on
    /// failure is backend-defined; the built-in memory backing preserves it.
    fn read_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error>;

    /// Writes one contiguous caller-owned slice.
    ///
    /// # Errors
    /// Returns the backend's validation or write error. Failure effects are
    /// backend-defined; the built-in memory backing rejects before changing bytes.
    fn write_with_sideband(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error>;

    /// Checks an extent on a unit-sideband backend.
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), Self::Error>
    where
        S: unit::UnitSideband,
    {
        self.validate_with_sideband(&S::unit(), address, alignment, length)
    }

    /// Reads bytes from a unit-sideband backend.
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        destination: &mut [u8],
    ) -> Result<(), Self::Error>
    where
        S: unit::UnitSideband,
    {
        self.read_with_sideband(&S::unit(), address, alignment, destination)
    }

    /// Writes bytes to a unit-sideband backend.
    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        source: &[u8],
    ) -> Result<(), Self::Error>
    where
        S: unit::UnitSideband,
    {
        self.write_with_sideband(&S::unit(), address, alignment, source)
    }

    /// Reads into a default-constructed byte buffer and returns its owner.
    ///
    /// The buffer's slice length determines the access extent. This does not
    /// decode values, infer alignment, or reinterpret Rust object layout.
    fn read_buffer<T>(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
    ) -> Result<T, Self::Error>
    where
        Self: Sized,
        S: unit::UnitSideband,
        T: Default + AsMut<[u8]>,
    {
        self.read_buffer_with_sideband(&S::unit(), address, alignment)
    }

    /// Writes a value's exposed bytes without encoding or reinterpreting it.
    fn write_buffer<T>(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
        S: unit::UnitSideband,
        T: AsRef<[u8]> + ?Sized,
    {
        self.write_buffer_with_sideband(&S::unit(), address, alignment, value)
    }

    /// Reads a byte-buffer owner using explicit access metadata.
    fn read_buffer_with_sideband<T>(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
    ) -> Result<T, Self::Error>
    where
        Self: Sized,
        T: Default + AsMut<[u8]>,
    {
        let mut value = T::default();
        self.read_with_sideband(sideband, address, alignment, value.as_mut())?;
        Ok(value)
    }

    /// Writes exposed bytes using explicit access metadata.
    fn write_buffer_with_sideband<T>(
        &self,
        sideband: &S,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
        T: AsRef<[u8]> + ?Sized,
    {
        self.write_with_sideband(sideband, address, alignment, value.as_ref())
    }
}
