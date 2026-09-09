use core::ops::Range;

use crate::{Documentation, SchemaCodecError};

use super::{FieldDefinition, RecordSpec};

/// Resolved scalar or nested-record member metadata.
#[derive(Debug, Clone, Copy)]
pub enum ResolvedMemberKind<I: 'static = u16> {
    /// One scalar placement.
    Field {
        /// Sibling-local identifier.
        id: I,
        /// Reusable scalar definition.
        definition: FieldDefinition,
    },
    /// One fixed or repeated nested-record instance.
    Record {
        /// Sibling-local identifier.
        id: I,
        /// Referenced record definition.
        definition: &'static RecordSpec<I>,
        /// Zero-based placement instance.
        instance: usize,
    },
}

/// One member with a deterministic root-qualified path and absolute range.
#[derive(Debug, Clone)]
pub struct ResolvedMember<I: 'static = u16> {
    pub(super) documentation: Documentation,
    pub(super) path: String,
    pub(super) absolute_offset: usize,
    pub(super) width: usize,
    pub(super) declaration_index: usize,
    pub(super) kind: ResolvedMemberKind<I>,
}

impl<I: Copy> ResolvedMember<I> {
    /// Returns effective documentation of this placed member.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }

    /// Returns the root-qualified rendered path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the absolute byte offset from the root record.
    pub const fn absolute_offset(&self) -> usize {
        self.absolute_offset
    }

    /// Returns the exact absolute byte range.
    pub const fn range(&self) -> Range<usize> {
        self.absolute_offset..self.absolute_offset + self.width
    }

    /// Returns the resolved member kind.
    pub const fn kind(&self) -> ResolvedMemberKind<I> {
        self.kind
    }

    /// Returns a checked immutable member subview.
    pub fn view<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8], SchemaCodecError> {
        let required = self.absolute_offset + self.width;
        bytes
            .get(self.range())
            .ok_or(SchemaCodecError::BufferTooShort {
                required,
                actual: bytes.len(),
            })
    }

    /// Returns a checked mutable member subview.
    pub fn view_mut<'a>(&self, bytes: &'a mut [u8]) -> Result<&'a mut [u8], SchemaCodecError> {
        let actual = bytes.len();
        let required = self.absolute_offset + self.width;
        bytes
            .get_mut(self.range())
            .ok_or(SchemaCodecError::BufferTooShort { required, actual })
    }
}

/// Fully validated record graph in deterministic depth-first order.
#[derive(Debug, Clone)]
pub struct ResolvedRecord<I: 'static = u16> {
    pub(super) documentation: Documentation,
    pub(super) root: &'static str,
    pub(super) size: usize,
    pub(super) alignment: usize,
    pub(super) members: Vec<ResolvedMember<I>>,
}

impl<I: Copy> ResolvedRecord<I> {
    /// Returns the root definition documentation.
    pub const fn documentation(&self) -> Documentation {
        self.documentation
    }

    /// Returns the root definition name.
    pub const fn root(&self) -> &'static str {
        self.root
    }

    /// Returns the root record byte size.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns the root record byte alignment.
    pub const fn alignment(&self) -> usize {
        self.alignment
    }

    /// Returns resolved members in depth-first declaration order.
    pub fn members(&self) -> &[ResolvedMember<I>] {
        &self.members
    }

    /// Returns a checked immutable root-record view.
    pub fn view<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8], SchemaCodecError> {
        bytes
            .get(..self.size)
            .ok_or(SchemaCodecError::BufferTooShort {
                required: self.size,
                actual: bytes.len(),
            })
    }

    /// Returns a checked mutable root-record view.
    pub fn view_mut<'a>(&self, bytes: &'a mut [u8]) -> Result<&'a mut [u8], SchemaCodecError> {
        let actual = bytes.len();
        bytes
            .get_mut(..self.size)
            .ok_or(SchemaCodecError::BufferTooShort {
                required: self.size,
                actual,
            })
    }
}
