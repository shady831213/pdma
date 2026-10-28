/// Stable schema-local identity of one declared member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaMemberIdentity {
    owner: &'static str,
    member: &'static str,
}

impl SchemaMemberIdentity {
    /// Creates one schema-local member identity.
    pub const fn new(owner: &'static str, member: &'static str) -> Self {
        Self { owner, member }
    }

    /// Returns the generated type that owns the member.
    pub const fn owner(self) -> &'static str {
        self.owner
    }

    /// Returns the declared member name.
    pub const fn member(self) -> &'static str {
        self.member
    }

    /// Places this identity at one root-relative byte coordinate.
    pub const fn at(self, offset: usize) -> SchemaLocation {
        self.at_byte(offset)
    }

    /// Places this identity at one root-relative byte coordinate.
    pub const fn at_byte(self, offset: usize) -> SchemaLocation {
        SchemaLocation::new(self, SchemaCoordinate::Byte(offset))
    }

    /// Places this identity at one root-relative bit coordinate.
    pub const fn at_bit(self, offset: usize) -> SchemaLocation {
        SchemaLocation::new(self, SchemaCoordinate::Bit(offset))
    }
}

/// Root-relative coordinate of one schema member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaCoordinate {
    /// Byte offset from the root layout.
    Byte(usize),
    /// Bit offset from the root layout.
    Bit(usize),
}

impl SchemaCoordinate {
    /// Rebases this local coordinate under `base` without wrapping.
    pub const fn checked_rebase(self, base: Self) -> Option<Self> {
        match (self, base) {
            (Self::Byte(local), Self::Byte(parent)) => match parent.checked_add(local) {
                Some(offset) => Some(Self::Byte(offset)),
                None => None,
            },
            (Self::Bit(local), Self::Byte(parent)) => match parent.checked_mul(8) {
                Some(parent) => match parent.checked_add(local) {
                    Some(offset) => Some(Self::Bit(offset)),
                    None => None,
                },
                None => None,
            },
            (Self::Byte(local), Self::Bit(parent)) => match local.checked_mul(8) {
                Some(local) => match parent.checked_add(local) {
                    Some(offset) => Some(Self::Bit(offset)),
                    None => None,
                },
                None => None,
            },
            (Self::Bit(local), Self::Bit(parent)) => match parent.checked_add(local) {
                Some(offset) => Some(Self::Bit(offset)),
                None => None,
            },
        }
    }
}

/// Stable member identity paired with its root-relative coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaLocation {
    identity: SchemaMemberIdentity,
    coordinate: SchemaCoordinate,
}

impl SchemaLocation {
    /// Creates one located schema member.
    pub const fn new(identity: SchemaMemberIdentity, coordinate: SchemaCoordinate) -> Self {
        Self {
            identity,
            coordinate,
        }
    }

    /// Returns the schema-local member identity.
    pub const fn identity(self) -> SchemaMemberIdentity {
        self.identity
    }

    /// Returns the root-relative coordinate.
    pub const fn coordinate(self) -> SchemaCoordinate {
        self.coordinate
    }

    /// Rebases this location under `base` without changing its leaf identity.
    pub const fn checked_rebase(self, base: SchemaCoordinate) -> Option<Self> {
        match self.coordinate.checked_rebase(base) {
            Some(coordinate) => Some(Self::new(self.identity, coordinate)),
            None => None,
        }
    }
}
