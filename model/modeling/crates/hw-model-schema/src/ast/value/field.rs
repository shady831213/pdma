use crate::{BitsFieldSpec, ast::Reference};
/// A packed field occurrence and its optional defined enumeration type.
#[derive(Debug, Clone, Copy)]
pub struct BitField<'a, I: 'static = u16> {
    spec: &'a BitsFieldSpec<I>,
    reference: Option<&'a Reference>,
}
impl<'a, I: Copy> BitField<'a, I> {
    pub(crate) const fn new(spec: &'a BitsFieldSpec<I>, reference: Option<&'a Reference>) -> Self {
        Self { spec, reference }
    }
    /// Physical field metadata, independent of its canonical type edge.
    pub const fn spec(self) -> &'a BitsFieldSpec<I> {
        self.spec
    }
    /// Defined type edge; absent for builtin packed integers.
    pub const fn reference(self) -> Option<&'a Reference> {
        self.reference
    }
    pub(crate) fn children(self) -> std::vec::IntoIter<super::Node<'a>> {
        self.reference
            .map_or_else(
                || {
                    self.spec
                        .definition()
                        .enum_definition()
                        .map(|_| {
                            super::Node::Enum(super::Enumeration::Bits(self.spec.definition()))
                        })
                        .into_iter()
                        .collect()
                },
                |reference| vec![super::Node::Reference(reference)],
            )
            .into_iter()
    }
}
impl<I> core::ops::Deref for BitField<'_, I> {
    type Target = BitsFieldSpec<I>;
    fn deref(&self) -> &Self::Target {
        self.spec
    }
}
