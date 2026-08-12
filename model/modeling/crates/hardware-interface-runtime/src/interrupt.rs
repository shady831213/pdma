mod dispatcher;
mod error;
mod observer;
mod registry;
mod traits;

pub use dispatcher::{
    InterruptDispatcher, InterruptDispatcherTask, InterruptShutdown, InterruptSummary,
};
pub use error::{InterruptDispatchError, InterruptJoinError};
pub use observer::{
    InterruptLifecycleEvent, InterruptLifecycleObserver, NoopInterruptObserver,
    SharedInterruptObserver,
};
pub use registry::{DuplicateHandler, EmptyRegistry, FrozenHandlerRegistry, HandlerRegistry};
pub use traits::{InterruptEnvelope, InterruptHandler, InterruptSource};
