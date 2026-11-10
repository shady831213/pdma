use crate::ast;

/// Physical record geometry without storage members.
#[derive(Clone, Copy)]
pub struct Storage(pub(crate) crate::RecordSpec);
impl Storage {
    /// Declared name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Encoded byte size.
    pub const fn size(self) -> usize {
        self.0.size()
    }
    /// Required byte alignment.
    pub const fn alignment(self) -> usize {
        self.0.alignment()
    }
    /// Declaration documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
}
/// Scalar encoding without enum or bits children.
#[derive(Clone, Copy)]
pub struct Scalar(pub(crate) crate::FieldDefinition);
impl Scalar {
    /// Encoded byte width.
    pub const fn width(self) -> usize {
        self.0.width()
    }
    /// Storage scalar kind.
    pub const fn scalar(self) -> crate::ScalarKind {
        self.0.scalar()
    }
    /// Byte order.
    pub const fn endian(self) -> crate::Endian {
        self.0.endian()
    }
    /// Definition documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
}
/// Enumeration marker; alternatives are delivered as child visits.
#[derive(Clone, Copy)]
pub struct Enumeration(pub(crate) ast::value::Enumeration);
impl Enumeration {
    /// Number of declared alternatives.
    pub const fn variant_count(self) -> usize {
        self.0.variants().len()
    }
    /// Effective encoding at this occurrence; byte order belongs to its embedding.
    pub const fn encoding(self) -> EnumEncoding {
        match self.0 {
            ast::value::Enumeration::Scalar(v) => EnumEncoding::Scalar(Scalar(v)),
            ast::value::Enumeration::Bits(v) => EnumEncoding::Bits {
                bit_width: v.bit_width(),
            },
        }
    }
}
/// Scalar or packed encoding carried by an enumeration occurrence.
#[derive(Debug, Clone, Copy)]
pub enum EnumEncoding {
    /// Byte-addressed scalar.
    Scalar(Scalar),
    /// Packed bit value.
    Bits {
        /// Number of encoded bits.
        bit_width: usize,
    },
}
/// Borrowed scalar bits geometry without field access.
#[derive(Clone, Copy)]
pub struct Bits<'a>(pub(crate) &'a ast::BitsDefinition, pub(crate) crate::Endian);
impl Bits<'_> {
    /// Scalar encoding at this placement, with its effective byte order.
    pub const fn encoding(self) -> Scalar {
        Scalar(crate::FieldDefinition::new(self.0.scalar()).with_endian(self.1))
    }
    /// Storage scalar kind.
    pub const fn scalar(self) -> crate::ScalarKind {
        self.0.scalar()
    }
    /// Highest exclusive bit endpoint.
    pub const fn bit_width(self) -> usize {
        self.0.bit_width()
    }
    /// Mask of declared bits.
    pub const fn declared_mask(self) -> u64 {
        self.0.declared_mask()
    }
}
/// Bit-field placement without its enumeration subtree or source ID type.
#[derive(Clone, Copy)]
pub struct BitField(crate::BitsFieldSpec<usize>);
impl BitField {
    pub(crate) fn from_spec<I: Copy + Into<usize>>(spec: &crate::BitsFieldSpec<I>) -> Self {
        Self(
            crate::BitsFieldSpec::place(
                spec.id().into(),
                spec.name(),
                spec.lsb(),
                spec.definition(),
            )
            .with_documentation(spec.documentation()),
        )
    }
    /// Sibling-local identity.
    pub const fn id(self) -> usize {
        self.0.id()
    }
    /// Declared field name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Least significant bit position.
    pub const fn lsb(self) -> usize {
        self.0.lsb()
    }
    /// Declared field width.
    pub const fn bit_width(self) -> usize {
        self.0.bit_width()
    }
    /// Placement documentation with definition fallback.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
    /// Checked local bit range.
    pub fn range(self) -> Option<core::ops::Range<usize>> {
        self.0.range()
    }
}
/// Borrowed canonical storage member without access to its child declarations.
#[derive(Clone, Copy)]
pub struct StorageMember<'a>(pub(crate) &'a crate::ResolvedMember);
impl<'a> StorageMember<'a> {
    /// Root-qualified source path.
    pub fn path(self) -> &'a str {
        self.0.path()
    }
    /// Absolute byte offset from the root record.
    pub const fn absolute_offset(self) -> usize {
        self.0.absolute_offset()
    }
    /// Encoded byte width.
    pub const fn width(self) -> usize {
        match self.0.kind() {
            crate::ResolvedMemberKind::Field { definition, .. } => definition.width(),
            crate::ResolvedMemberKind::Record { definition, .. } => definition.size(),
        }
    }
    /// Sibling-local identity.
    pub const fn id(self) -> u16 {
        match self.0.kind() {
            crate::ResolvedMemberKind::Field { id, .. }
            | crate::ResolvedMemberKind::Record { id, .. } => id,
        }
    }
    /// Nested record instance index, absent for scalar members.
    pub const fn instance(self) -> Option<usize> {
        match self.0.kind() {
            crate::ResolvedMemberKind::Field { .. } => None,
            crate::ResolvedMemberKind::Record { instance, .. } => Some(instance),
        }
    }
    /// Placement documentation with definition fallback.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
    /// Whether this member places a nested record envelope.
    pub const fn is_record(self) -> bool {
        matches!(self.0.kind(), crate::ResolvedMemberKind::Record { .. })
    }
}

impl core::fmt::Debug for Storage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Storage")
            .field("name", &self.name())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Scalar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Scalar")
            .field("scalar", &self.scalar())
            .field("width", &self.width())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Enumeration {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Enumeration")
            .field("variant_count", &self.variant_count())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Bits<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bits")
            .field("scalar", &self.scalar())
            .field("bit_width", &self.bit_width())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for StorageMember<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StorageMember")
            .field("path", &self.path())
            .field("offset", &self.absolute_offset())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for BitField {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BitField")
            .field("name", &self.name())
            .field("lsb", &self.lsb())
            .finish_non_exhaustive()
    }
}
