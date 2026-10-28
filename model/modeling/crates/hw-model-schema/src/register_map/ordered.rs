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
    bank: bool,
    documentation: crate::Documentation,
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
            bank: false,
            name,
            offset,
            count,
            stride,
            alignment: geometry.alignment,
            documentation: crate::Documentation::EMPTY,
            extent: geometry.extent,
        }
    }

    /// Attaches documentation to this node.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: crate::Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns documentation of this node.
    pub const fn documentation(self) -> crate::Documentation {
        self.documentation
    }

    /// Returns the generated placement identity.
    pub const fn id(self) -> I {
        self.id
    }

    /// Preserves explicit bank identity, including a count of one.
    #[must_use]
    pub const fn as_bank(mut self) -> Self {
        self.bank = true;
        self
    }

    /// Returns whether brackets declared this placement as a bank.
    pub const fn is_bank(self) -> bool {
        self.bank
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
    documentation: crate::Documentation,
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
            documentation: crate::Documentation::EMPTY,
        }
    }

    /// Attaches documentation to this node.
    #[must_use]
    pub const fn with_documentation(mut self, documentation: crate::Documentation) -> Self {
        self.documentation = documentation;
        self
    }

    /// Returns documentation of this node.
    pub const fn documentation(self) -> crate::Documentation {
        self.documentation
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

#[doc(hidden)]
#[derive(Debug)]
pub struct RegisterCollection<'a, I: 'static> {
    #[doc(hidden)]
    pub placements: &'a mut Vec<super::ResolvedRegisterMapPlacement<I>>,
    #[doc(hidden)]
    pub documentation: crate::Documentation,
    #[doc(hidden)]
    pub id: I,
    #[doc(hidden)]
    pub name: &'static str,
    #[doc(hidden)]
    pub path: String,
    #[doc(hidden)]
    pub offset: usize,
    #[doc(hidden)]
    pub declaration_index: &'a mut usize,
    #[doc(hidden)]
    pub repeated: Option<(&'static str, usize)>,
    #[doc(hidden)]
    pub output: &'a mut Vec<ResolvedRegister<I>>,
}

#[doc(hidden)]
pub fn append_register_leaf<I: Copy + 'static>(
    collection: RegisterCollection<'_, I>,
    definition: RegisterDefinition<u16>,
) {
    let index = *collection.declaration_index;
    let mut register = RegisterSpec::place(
        collection.id,
        collection.name,
        collection.offset,
        definition,
    )
    .with_declaration_index(index)
    .with_documentation(collection.documentation);
    if let Ok(numeric_id) = u16::try_from(index) {
        register = register.with_numeric_id(numeric_id);
    }
    collection.output.push(ResolvedRegister::ordered(
        collection.path,
        collection.offset,
        index,
        register,
        collection.repeated,
    ));
    *collection.declaration_index = collection.declaration_index.saturating_add(1);
}

#[doc(hidden)]
pub fn finish_ordered_register_map<I>(
    root: &'static str,
    extent: usize,
    registers: Vec<ResolvedRegister<I>>,
) -> Result<ResolvedRegisterMap<I>, RegisterMapError<I>>
where
    I: Copy + Eq + fmt::Debug + 'static,
{
    for (position, register) in registers.iter().enumerate() {
        register
            .definition()
            .validate_at(root, register.path(), register.declaration_index())
            .map_err(RegisterMapError::Definition)?;
        let end = register
            .offset()
            .checked_add(register.definition().width())
            .ok_or_else(|| RegisterMapError::RegisterOutOfBounds {
                root,
                path: register.path().to_owned(),
                declaration_index: register.declaration_index(),
                offset: register.offset(),
                width: register.definition().width(),
                map_size: extent,
            })?;
        if end > extent {
            return Err(RegisterMapError::RegisterOutOfBounds {
                root,
                path: register.path().to_owned(),
                declaration_index: register.declaration_index(),
                offset: register.offset(),
                width: register.definition().width(),
                map_size: extent,
            });
        }
        for previous in registers.iter().take(position) {
            if previous.offset() == register.offset() {
                return Err(RegisterMapError::DuplicateExpandedStart {
                    root,
                    path: register.path().to_owned(),
                    declaration_index: register.declaration_index(),
                    offset: register.offset(),
                    first_path: previous.path().to_owned(),
                });
            }
            let previous_end = previous.offset() + previous.definition().width();
            if previous.offset() < end && register.offset() < previous_end {
                return Err(RegisterMapError::ExpandedOverlap {
                    root,
                    path: register.path().to_owned(),
                    declaration_index: register.declaration_index(),
                    first_path: previous.path().to_owned(),
                });
            }
            if previous.path() == register.path() {
                return Err(RegisterMapError::ResolvedPathCollision {
                    root,
                    path: register.path().to_owned(),
                    declaration_index: register.declaration_index(),
                    first_declaration_index: previous.declaration_index(),
                });
            }
        }
    }
    Ok(ResolvedRegisterMap::ordered(root, extent, registers))
}
use core::fmt;

use crate::register::{RegisterDefinition, RegisterSpec};

use super::{RegisterMapError, ResolvedRegister, ResolvedRegisterMap};
