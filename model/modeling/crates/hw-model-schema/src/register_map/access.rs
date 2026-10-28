/// One named namespace in a register access path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterAccessGroup {
    name: &'static str,
}

impl RegisterAccessGroup {
    /// Creates runtime-neutral group metadata.
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    /// Returns the declared group name.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

/// The named leaf and placement target of a register access path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterAccessLeaf<I> {
    name: &'static str,
    target: I,
}

impl<I: Copy> RegisterAccessLeaf<I> {
    /// Creates runtime-neutral leaf metadata.
    pub const fn new(name: &'static str, target: I) -> Self {
        Self { name, target }
    }

    /// Returns the declared leaf name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the targeted root placement identity.
    pub const fn target(self) -> I {
        self.target
    }
}

/// One explicit access-only path to a root register-map placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterAccessPath<I: 'static> {
    groups: &'static [RegisterAccessGroup],
    leaf: RegisterAccessLeaf<I>,
}

impl<I: Copy + 'static> RegisterAccessPath<I> {
    /// Creates one runtime-neutral path descriptor.
    pub const fn new(groups: &'static [RegisterAccessGroup], leaf: RegisterAccessLeaf<I>) -> Self {
        Self { groups, leaf }
    }

    /// Returns namespace groups from root to leaf parent.
    pub const fn groups(self) -> &'static [RegisterAccessGroup] {
        self.groups
    }

    /// Returns leaf metadata and its placement target.
    pub const fn leaf(self) -> RegisterAccessLeaf<I> {
        self.leaf
    }
}
