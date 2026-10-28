#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod env;
mod interrupt;
mod notifications;
mod trace;

pub use env::{EnvAccessError, EnvContextError, EnvLocal, EnvRegisterAccess, SoftwareEnv};
pub use interrupt::{
    DuplicateHandler, EmptyRegistry, FrozenHandlerRegistry, HandlerRegistry,
    InterruptDispatchError, InterruptDispatcher, InterruptDispatcherTask, InterruptJoinError,
    InterruptLifecycleEvent, InterruptLifecycleObserver, InterruptRegistration,
    InterruptRegistrationError, InterruptRuntime, InterruptShutdown, InterruptShutdownReceiver,
    InterruptSummary, NoopInterruptObserver, SharedInterruptObserver,
};
pub use notifications::{NotificationWait, Notifications, Notifier, notifications};
pub use trace::{TraceBuffer, TraceEntry};
