use super::RecordSpec;

#[derive(Debug, Clone, Copy)]
enum RecordReference<I: 'static> {
    Direct(&'static RecordSpec<I>),
    Deferred(fn() -> &'static RecordSpec<I>),
}

/// One fixed or repeated sibling-local nested record placement.
#[derive(Debug, Clone, Copy)]
pub struct RecordPlacement<I: 'static = u16> {
    id: I,
    name: &'static str,
    offset: usize,
    count: usize,
    stride: usize,
    definition: RecordReference<I>,
    declaration_index: Option<usize>,
}

impl<I: Copy> RecordPlacement<I> {
    /// Places one nested record definition.
    pub const fn new(
        id: I,
        name: &'static str,
        offset: usize,
        definition: &'static RecordSpec<I>,
    ) -> Self {
        Self::direct(id, name, offset, 1, 0, definition)
    }

    /// Places repeated instances of one nested record definition.
    pub const fn repeated(
        id: I,
        name: &'static str,
        offset: usize,
        count: usize,
        stride: usize,
        definition: &'static RecordSpec<I>,
    ) -> Self {
        Self::direct(id, name, offset, count, stride, definition)
    }

    /// Places a lazily referenced definition, permitting recursion validation.
    pub const fn deferred(
        id: I,
        name: &'static str,
        offset: usize,
        definition: fn() -> &'static RecordSpec<I>,
    ) -> Self {
        Self {
            id,
            name,
            offset,
            count: 1,
            stride: 0,
            definition: RecordReference::Deferred(definition),
            declaration_index: None,
        }
    }

    const fn direct(
        id: I,
        name: &'static str,
        offset: usize,
        count: usize,
        stride: usize,
        definition: &'static RecordSpec<I>,
    ) -> Self {
        Self {
            id,
            name,
            offset,
            count,
            stride,
            definition: RecordReference::Direct(definition),
            declaration_index: None,
        }
    }

    /// Selects an explicit combined sibling declaration index.
    #[must_use]
    pub const fn with_declaration_index(mut self, index: usize) -> Self {
        self.declaration_index = Some(index);
        self
    }

    /// Returns the sibling identifier.
    pub const fn id(self) -> I {
        self.id
    }

    /// Returns the sibling name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the first instance's local byte offset.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Returns the instance count.
    pub const fn count(self) -> usize {
        self.count
    }

    /// Returns the repeated-instance stride.
    pub const fn stride(self) -> usize {
        self.stride
    }

    /// Returns the referenced record definition.
    pub fn definition(self) -> &'static RecordSpec<I> {
        match self.definition {
            RecordReference::Direct(definition) => definition,
            RecordReference::Deferred(definition) => definition(),
        }
    }

    pub(super) const fn declared_at(self, fallback: usize) -> usize {
        match self.declaration_index {
            Some(index) => index,
            None => fallback,
        }
    }
}
