use super::{
    error::{InterruptDispatchError, InterruptJoinError},
    observer::{InterruptLifecycleEvent, SharedInterruptObserver},
    registry::{FrozenHandlerRegistry, HandlerRegistry},
    traits::{InterruptEnvelope, InterruptHandler, InterruptSource},
};
use crate::SoftwareCpu;
use std::{fmt, sync::Arc};
use tokio::{
    sync::{Mutex, watch},
    task::JoinHandle,
};

type DispatchJoin<V, S, H> = JoinHandle<Result<InterruptSummary, InterruptDispatchError<V, S, H>>>;
type DispatchRegistry<S, H> = Mutex<
    HandlerRegistry<
        <<S as InterruptSource>::Notification as InterruptEnvelope>::Vector,
        <S as InterruptSource>::Notification,
        H,
    >,
>;

/// Stateless constructor namespace for supervised interrupt dispatch.
#[derive(Debug, Clone, Copy)]
pub struct InterruptDispatcher;

/// Final successful-delivery accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptSummary {
    deliveries: usize,
}

impl InterruptSummary {
    /// Number of handlers that completed successfully.
    pub const fn deliveries(self) -> usize {
        self.deliveries
    }
}

/// Cloneable signal-only cooperative shutdown capability.
#[derive(Debug, Clone)]
pub struct InterruptShutdown {
    shutdown: watch::Sender<bool>,
}

/// Sole-consumer shutdown signal paired with [`InterruptShutdown`].
#[derive(Debug)]
pub struct InterruptShutdownReceiver {
    pub(super) shutdown: watch::Receiver<bool>,
}

impl InterruptShutdown {
    /// Requests shutdown without joining or accessing actor-owned state.
    pub fn request_shutdown(&self) {
        self.shutdown.send_replace(true);
    }
}

/// Sole owner of dispatcher join authority.
pub struct InterruptDispatcherTask<V, S, H> {
    shutdown: InterruptShutdown,
    join: Option<DispatchJoin<V, S, H>>,
}

impl<V, S, H> fmt::Debug for InterruptDispatcherTask<V, S, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InterruptDispatcherTask")
            .field("shutdown", &self.shutdown)
            .field("join_owned", &self.join.is_some())
            .finish()
    }
}

impl<V, S, H> InterruptDispatcherTask<V, S, H> {
    /// Returns a cloneable signal-only shutdown capability.
    pub fn shutdown_handle(&self) -> InterruptShutdown {
        self.shutdown.clone()
    }

    /// Requests cooperative shutdown and joins owned state teardown.
    pub async fn shutdown_and_join(
        mut self,
    ) -> Result<InterruptSummary, InterruptJoinError<V, S, H>> {
        self.shutdown.request_shutdown();
        let join = self
            .join
            .take()
            .ok_or(InterruptJoinError::OwnershipConsumed)?;
        join.await
            .map_err(InterruptJoinError::Task)?
            .map_err(InterruptJoinError::Dispatch)
    }
}

impl<V, S, H> Drop for InterruptDispatcherTask<V, S, H> {
    fn drop(&mut self) {
        self.shutdown.request_shutdown();
    }
}

impl InterruptDispatcher {
    /// Creates one signal-only shutdown capability and its sole receiver.
    pub fn shutdown_channel() -> (InterruptShutdown, InterruptShutdownReceiver) {
        let (shutdown, receiver) = watch::channel(false);
        (
            InterruptShutdown { shutdown },
            InterruptShutdownReceiver { shutdown: receiver },
        )
    }

    /// Runs the canonical dispatcher in the caller-owned future.
    pub async fn run<S, H>(
        cpu: Arc<SoftwareCpu>,
        registry: FrozenHandlerRegistry<
            <<S as InterruptSource>::Notification as InterruptEnvelope>::Vector,
            S::Notification,
            H,
        >,
        source: S,
        observer: SharedInterruptObserver<S::Notification>,
        receiver: InterruptShutdownReceiver,
    ) -> Result<
        InterruptSummary,
        InterruptDispatchError<
            <<S as InterruptSource>::Notification as InterruptEnvelope>::Vector,
            S::Error,
            H::Error,
        >,
    >
    where
        S: InterruptSource,
        H: InterruptHandler<S::Notification>,
    {
        let registry = Mutex::new(registry.into_registry());
        run_dispatch(cpu, &registry, source, observer, receiver.shutdown).await
    }

    /// Freezes a non-empty registry and starts its sole-owner dispatcher task.
    pub fn start<S, H>(
        cpu: Arc<SoftwareCpu>,
        registry: FrozenHandlerRegistry<
            <<S as InterruptSource>::Notification as InterruptEnvelope>::Vector,
            S::Notification,
            H,
        >,
        source: S,
        observer: SharedInterruptObserver<S::Notification>,
    ) -> InterruptDispatcherTask<
        <<S as InterruptSource>::Notification as InterruptEnvelope>::Vector,
        S::Error,
        H::Error,
    >
    where
        S: InterruptSource,
        H: InterruptHandler<S::Notification> + 'static,
    {
        let (shutdown, receiver) = Self::shutdown_channel();
        let join = tokio::spawn(Self::run(cpu, registry, source, observer, receiver));
        InterruptDispatcherTask {
            shutdown,
            join: Some(join),
        }
    }
}

pub(super) async fn run_dispatch<S, H>(
    cpu: Arc<SoftwareCpu>,
    registry: &DispatchRegistry<S, H>,
    mut source: S,
    observer: SharedInterruptObserver<S::Notification>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<
    InterruptSummary,
    InterruptDispatchError<
        <<S as InterruptSource>::Notification as InterruptEnvelope>::Vector,
        S::Error,
        H::Error,
    >,
>
where
    S: InterruptSource,
    H: InterruptHandler<S::Notification>,
{
    let mut deliveries = 0usize;
    loop {
        let ready = tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            result = source.wait_ready() => result.map_err(InterruptDispatchError::Readiness)?,
        };
        if *shutdown.borrow() {
            break;
        }
        let claimed = tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            result = source.claim(ready) => result.map_err(InterruptDispatchError::Claim)?,
        };
        let Some(notification) = claimed else {
            continue;
        };
        let vector = notification.vector();
        if !registry.lock().await.handlers.contains_key(&vector) {
            return Err(InterruptDispatchError::UnregisteredVector(vector));
        }
        cpu.run_handler_turn(async {
            let mut registry = registry.lock().await;
            let handler = registry
                .handlers
                .get_mut(&vector)
                .ok_or_else(|| InterruptDispatchError::UnregisteredVector(vector.clone()))?;
            observer.observe(InterruptLifecycleEvent::Started {
                notification: &notification,
                vector: &vector,
            });
            let result = handler.handle(&notification).await;
            drop(registry);
            match &result {
                Ok(()) => observer.observe(InterruptLifecycleEvent::Succeeded {
                    notification: &notification,
                    vector: &vector,
                }),
                Err(error) => observer.observe(InterruptLifecycleEvent::Failed {
                    notification: &notification,
                    vector: &vector,
                    error,
                }),
            }
            result.map_err(|source| InterruptDispatchError::Handler { vector, source })
        })
        .await?;
        deliveries = deliveries.saturating_add(1);
    }
    Ok(InterruptSummary { deliveries })
}
