#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod cpu;
mod interrupt;
mod notifications;
mod trace;

pub use cpu::{CpuAccessError, CpuContextError, CpuLocal, CpuRegisterAccess, SoftwareCpu};
pub use interrupt::{
    DuplicateHandler, EmptyRegistry, FrozenHandlerRegistry, HandlerRegistry,
    InterruptDispatchError, InterruptDispatcher, InterruptDispatcherTask, InterruptJoinError,
    InterruptLifecycleEvent, InterruptLifecycleObserver, InterruptRegistration,
    InterruptRegistrationError, InterruptRuntime, InterruptShutdown, InterruptShutdownReceiver,
    InterruptSummary, NoopInterruptObserver, SharedInterruptObserver,
};
pub use notifications::{NotificationWait, Notifications, Notifier, notifications};
pub use trace::TraceBuffer;
