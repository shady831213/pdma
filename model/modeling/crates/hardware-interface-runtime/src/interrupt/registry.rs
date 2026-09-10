use super::traits::InterruptHandler;
use std::{collections::BTreeMap, fmt, marker::PhantomData};

/// Duplicate registration result retaining the rejected handler.
#[derive(Debug)]
pub struct DuplicateHandler<V, H> {
    vector: V,
    handler: H,
}

impl<V, H> DuplicateHandler<V, H> {
    /// Returns the duplicate vector.
    pub const fn vector(&self) -> &V {
        &self.vector
    }

    /// Recovers the handler that was not installed.
    pub fn into_handler(self) -> H {
        self.handler
    }
}

/// Mutable build-time vector registry with one concrete handler representation.
pub struct HandlerRegistry<V, N, H> {
    pub(super) handlers: BTreeMap<V, H>,
    notification: PhantomData<fn(N)>,
}

impl<V, N, H> HandlerRegistry<V, N, H> {
    /// Creates an empty mutable registry.
    pub const fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
            notification: PhantomData,
        }
    }

    /// Creates a non-empty registry from one initial handler.
    pub fn with_handler(vector: V, handler: H) -> Self
    where
        V: Ord,
    {
        let mut handlers = BTreeMap::new();
        handlers.insert(vector, handler);
        Self {
            handlers,
            notification: PhantomData,
        }
    }

    /// Reports whether no handler has been registered.
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// Freezes a non-empty registry or returns the unchanged builder.
    pub fn freeze(self) -> Result<FrozenHandlerRegistry<V, N, H>, EmptyRegistry<V, N, H>> {
        if self.handlers.is_empty() {
            Err(EmptyRegistry { registry: self })
        } else {
            Ok(FrozenHandlerRegistry {
                handlers: self.handlers,
                notification: PhantomData,
            })
        }
    }
}

impl<V, N, H> HandlerRegistry<V, N, H>
where
    V: Ord,
    H: InterruptHandler<N>,
{
    /// Registers one handler without replacing an existing entry.
    pub fn register(&mut self, vector: V, handler: H) -> Result<(), DuplicateHandler<V, H>> {
        if self.handlers.contains_key(&vector) {
            return Err(DuplicateHandler { vector, handler });
        }
        self.handlers.insert(vector, handler);
        Ok(())
    }
}

impl<V, N, H> Default for HandlerRegistry<V, N, H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: fmt::Debug, N, H> fmt::Debug for HandlerRegistry<V, N, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandlerRegistry")
            .field("vectors", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Recoverable empty-freeze error retaining the original builder.
pub struct EmptyRegistry<V, N, H> {
    registry: HandlerRegistry<V, N, H>,
}

impl<V: fmt::Debug, N, H> fmt::Debug for EmptyRegistry<V, N, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("EmptyRegistry")
            .field(&self.registry)
            .finish()
    }
}

impl<V, N, H> EmptyRegistry<V, N, H> {
    /// Recovers the unchanged mutable registry.
    pub fn into_registry(self) -> HandlerRegistry<V, N, H> {
        self.registry
    }
}

/// Immutable-start registry consumed exactly once by a dispatcher task.
pub struct FrozenHandlerRegistry<V, N, H> {
    pub(super) handlers: BTreeMap<V, H>,
    notification: PhantomData<fn(N)>,
}

impl<V, N, H> FrozenHandlerRegistry<V, N, H> {
    pub(super) fn into_registry(self) -> HandlerRegistry<V, N, H> {
        HandlerRegistry {
            handlers: self.handlers,
            notification: PhantomData,
        }
    }
}

impl<V: fmt::Debug, N, H> fmt::Debug for FrozenHandlerRegistry<V, N, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FrozenHandlerRegistry")
            .field("vectors", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}
