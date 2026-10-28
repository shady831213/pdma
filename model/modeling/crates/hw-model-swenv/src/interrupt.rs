mod dispatcher;
mod error;
mod observer;
mod registration;
mod registry;
mod runtime;

pub use dispatcher::{
    InterruptDispatcher, InterruptDispatcherTask, InterruptShutdown, InterruptShutdownReceiver,
    InterruptSummary,
};
pub use error::{InterruptDispatchError, InterruptJoinError};
pub use observer::{
    InterruptLifecycleEvent, InterruptLifecycleObserver, NoopInterruptObserver,
    SharedInterruptObserver,
};
pub use registration::{InterruptRegistration, InterruptRegistrationError};
pub use registry::{DuplicateHandler, EmptyRegistry, FrozenHandlerRegistry, HandlerRegistry};
pub use runtime::InterruptRuntime;
