use super::Definition;
use std::sync::Arc;

/// Why one schema definition refers to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A record member uses another definition.
    Field,
    /// A union alternative uses another definition.
    Variant,
    /// A register map places a child definition.
    Placement,
}

/// One reference edge; duplicate targets do not discard distinct field edges.
#[derive(Debug, Clone)]
pub struct Reference {
    name: &'static str,
    kind: ReferenceKind,
    target: Target,
    discriminant: Option<u64>,
    selector_binding: Option<&'static str>,
}

#[derive(Debug, Clone)]
enum Target {
    Factory(fn() -> Definition),
    Owned(Arc<Definition>),
}

impl Reference {
    /// Creates an edge whose target can be obtained without a global registry.
    pub const fn new(name: &'static str, kind: ReferenceKind, target: fn() -> Definition) -> Self {
        Self {
            name,
            kind,
            target: Target::Factory(target),
            discriminant: None,
            selector_binding: None,
        }
    }
    /// Creates an edge that owns the exact transformed target declaration.
    pub fn owned(name: &'static str, kind: ReferenceKind, target: Definition) -> Self {
        Self {
            name,
            kind,
            target: Target::Owned(Arc::new(target)),
            discriminant: None,
            selector_binding: None,
        }
    }
    /// Replaces the target while preserving edge metadata.
    pub(crate) fn retarget(&self, target: Definition) -> Self {
        Self {
            target: Target::Owned(Arc::new(target)),
            ..self.clone()
        }
    }
    /// Binds an external union selector in this reference's lexical owner scope.
    #[must_use]
    pub const fn with_selector_binding(mut self, selector: &'static str) -> Self {
        self.selector_binding = Some(selector);
        self
    }
    /// Selector path interpreted at this use site, never inside the reusable payload.
    pub const fn selector_binding(&self) -> Option<&'static str> {
        self.selector_binding
    }
    /// Attaches a tagged-union wire discriminant.
    #[must_use]
    pub const fn with_discriminant(mut self, value: u64) -> Self {
        self.discriminant = Some(value);
        self
    }
    /// Field, alternative or placement name in its owner.
    pub const fn name(&self) -> &'static str {
        self.name
    }
    /// Returns the relationship kind.
    pub const fn kind(&self) -> ReferenceKind {
        self.kind
    }
    /// Returns the selected alternative's wire value, when tagged.
    pub const fn discriminant(&self) -> Option<u64> {
        self.discriminant
    }
    /// Obtains the referenced definition, without following its own edges.
    pub fn target(&self) -> Definition {
        match &self.target {
            Target::Factory(factory) => factory(),
            Target::Owned(target) => target.as_ref().clone(),
        }
    }
}
