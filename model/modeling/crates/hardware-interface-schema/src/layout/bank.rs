use crate::{RegisterAccessError, RegisterCollection, RegisterLayout};

/// Invalid immutable register-bank geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterBankError {
    /// A bank must contain at least one instance.
    ZeroCount,
    /// Child alignment is invalid or the placement violates it.
    Alignment,
    /// Address arithmetic cannot be represented.
    Overflow,
    /// Complete block address envelopes overlap.
    Overlap,
    /// A child is empty or an explicit map extent excludes a child.
    Extent,
}

impl core::fmt::Display for RegisterBankError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid register bank: {self:?}")
    }
}
impl std::error::Error for RegisterBankError {}

/// Geometry shared by fixed and constructor-bound register layouts.
pub trait RegisterShape {
    /// Documentation of the reusable child layout.
    fn documentation(&self) -> crate::Documentation {
        crate::Documentation::EMPTY
    }
    /// Required placement alignment.
    fn alignment(&self) -> usize;
    /// Occupied extent including internal holes.
    fn extent(&self) -> usize;
    /// Collects the existing generated leaf metadata.
    fn append_registers<I: Copy + 'static>(&self, collection: RegisterCollection<'_, I>);
}

/// State-free adapter for a generated fixed register layout.
#[derive(Debug)]
pub struct StaticRegisterLayout<L>(core::marker::PhantomData<L>);

impl<L> Clone for StaticRegisterLayout<L> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<L> Copy for StaticRegisterLayout<L> {}

impl<L> StaticRegisterLayout<L> {
    /// Creates a fixed-layout geometry adapter.
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}
impl<L> Default for StaticRegisterLayout<L> {
    fn default() -> Self {
        Self::new()
    }
}
impl<L: RegisterLayout> RegisterShape for StaticRegisterLayout<L> {
    fn documentation(&self) -> crate::Documentation {
        L::DOCUMENTATION
    }
    fn alignment(&self) -> usize {
        L::ALIGNMENT
    }
    fn extent(&self) -> usize {
        L::EXTENT
    }
    fn append_registers<I: Copy + 'static>(&self, collection: RegisterCollection<'_, I>) {
        L::append_registers(collection);
    }
}

/// Immutable placement of repeated child geometry, independent of hardware state.
#[derive(Debug, Clone)]
pub struct RegisterBank<L> {
    child: L,
    base: usize,
    count: usize,
    stride: usize,
    end: usize,
}

impl<L: RegisterShape> RegisterBank<L> {
    /// Validates a bank before any metadata collection or hardware allocation.
    pub fn new(child: L, base: usize, count: usize) -> Result<Self, RegisterBankError> {
        if count == 0 {
            return Err(RegisterBankError::ZeroCount);
        }
        let alignment = child.alignment();
        if alignment == 0 || !alignment.is_power_of_two() || base % alignment != 0 {
            return Err(RegisterBankError::Alignment);
        }
        let stride = crate::checked_align_up(child.extent(), alignment)
            .ok_or(RegisterBankError::Overflow)?;
        if stride == 0 {
            return Err(RegisterBankError::Extent);
        }
        let end = crate::checked_placement_end(base, count, stride, child.extent())
            .ok_or(RegisterBankError::Overflow)?;
        Ok(Self {
            child,
            base,
            count,
            stride,
            end,
        })
    }
    /// Returns the first instance offset.
    pub const fn base(&self) -> usize {
        self.base
    }
    /// Returns the immutable instance count.
    pub const fn count(&self) -> usize {
        self.count
    }
    /// Returns the distance between child instances.
    pub const fn stride(&self) -> usize {
        self.stride
    }
    /// Returns the occupied bank extent, excluding final stride padding.
    pub const fn extent(&self) -> usize {
        self.end - self.base
    }
    /// Returns the first offset after the bank.
    pub const fn end(&self) -> usize {
        self.end
    }
    /// Returns the reusable child geometry.
    pub const fn child(&self) -> &L {
        &self.child
    }
    /// Returns the checked offset of an instance.
    pub const fn offset(&self, index: usize) -> Option<usize> {
        if index < self.count {
            checked_bank_offset(self.base, index, self.stride)
        } else {
            None
        }
    }
    /// Translates this immutable bank into a parent address space.
    pub fn rebased(&self, parent_base: usize) -> Result<Self, RegisterBankError>
    where
        L: Clone,
    {
        let base = self
            .base
            .checked_add(parent_base)
            .ok_or(RegisterBankError::Overflow)?;
        Self::new(self.child.clone(), base, self.count)
    }
    /// Locates a child aperture, including reserved holes but excluding alignment padding.
    pub fn locate(&self, address: usize) -> Option<(usize, usize)> {
        self.locate_instance(address)
            .filter(|(_, offset)| *offset < self.child.extent())
    }
    /// Selects a configured stride slot, including padding; the caller validates its aperture.
    pub fn locate_instance(&self, address: usize) -> Option<(usize, usize)> {
        let relative = address.checked_sub(self.base)?;
        let index = relative / self.stride;
        let offset = relative % self.stride;
        (index < self.count).then_some((index, offset))
    }
    /// Routes a child-aperture address to borrowed caller-owned hardware.
    pub fn route<T, E>(
        &self,
        address: usize,
        access: impl FnOnce(usize, usize) -> Result<T, RegisterAccessError<E>>,
    ) -> Result<T, RegisterAccessError<E>> {
        let (index, offset) = self
            .locate(address)
            .ok_or(RegisterAccessError::OutsideWindow { address })?;
        access(index, offset)
    }
    /// Produces the terminal miss for a bank after all sibling routes were tried.
    pub const fn finish_access<E>(&self, address: usize) -> RegisterAccessError<E> {
        match address.checked_sub(self.base) {
            Some(relative) if address < self.end => RegisterAccessError::InvalidOffset {
                offset: relative % self.stride,
            },
            _ => RegisterAccessError::OutsideWindow { address },
        }
    }
    /// Rejects overlapping complete bank envelopes, including internal holes.
    pub const fn validate_disjoint<R: RegisterShape>(
        &self,
        other: &RegisterBank<R>,
    ) -> Result<(), RegisterBankError> {
        if self.end <= other.base || other.end <= self.base {
            return Ok(());
        }
        Err(RegisterBankError::Overlap)
    }
}

/// Computes a bank instance offset without requiring a configured count.
pub const fn checked_bank_offset(base: usize, index: usize, stride: usize) -> Option<usize> {
    match index.checked_mul(stride) {
        Some(relative) => base.checked_add(relative),
        None => None,
    }
}
