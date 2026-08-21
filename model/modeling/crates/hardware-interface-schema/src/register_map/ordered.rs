/// Alignment and extent of one placed child layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterLayoutGeometry {
    alignment: usize,
    extent: usize,
}

impl RegisterLayoutGeometry {
    /// Creates child-layout geometry.
    pub const fn new(alignment: usize, extent: usize) -> Self {
        Self { alignment, extent }
    }
}

/// One ordered child-layout placement in a generated register map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterMapPlacement<I> {
    id: I,
    name: &'static str,
    offset: usize,
    count: usize,
    stride: usize,
    alignment: usize,
    extent: usize,
}

impl<I: Copy> RegisterMapPlacement<I> {
    /// Creates one checked-by-construction generated placement.
    pub const fn new(
        id: I,
        name: &'static str,
        offset: usize,
        count: usize,
        stride: usize,
        geometry: RegisterLayoutGeometry,
    ) -> Self {
        Self {
            id,
            name,
            offset,
            count,
            stride,
            alignment: geometry.alignment,
            extent: geometry.extent,
        }
    }

    /// Returns the generated placement identity.
    pub const fn id(self) -> I {
        self.id
    }

    /// Returns the placement name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the first instance offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the instance count.
    pub const fn count(self) -> usize {
        self.count
    }

    /// Returns the instance stride.
    pub const fn stride(self) -> usize {
        self.stride
    }

    /// Returns the child layout alignment.
    pub const fn alignment(self) -> usize {
        self.alignment
    }

    /// Returns one child instance extent.
    pub const fn extent(self) -> usize {
        self.extent
    }
}

/// Ordered root metadata generated for one register map.
#[derive(Debug, Clone, Copy)]
pub struct OrderedRegisterMapSpec<I: 'static> {
    name: &'static str,
    alignment: usize,
    extent: usize,
    placements: &'static [RegisterMapPlacement<I>],
}

impl<I: Copy> OrderedRegisterMapSpec<I> {
    /// Creates generated ordered map metadata.
    pub const fn new(
        name: &'static str,
        alignment: usize,
        extent: usize,
        placements: &'static [RegisterMapPlacement<I>],
    ) -> Self {
        Self {
            name,
            alignment,
            extent,
            placements,
        }
    }

    /// Returns the map name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the map alignment.
    pub const fn alignment(self) -> usize {
        self.alignment
    }

    /// Returns the map extent.
    pub const fn extent(self) -> usize {
        self.extent
    }

    /// Returns placements in declaration order.
    pub const fn placements(self) -> &'static [RegisterMapPlacement<I>] {
        self.placements
    }
}
