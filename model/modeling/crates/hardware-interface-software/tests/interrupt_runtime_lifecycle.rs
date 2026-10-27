//! Scoped runtime teardown and failure propagation through the real dispatcher.

use hardware_interface_irq::{InterruptEnvelope, InterruptHandler, InterruptSource};
use hardware_interface_software::{
    EnvContextError, InterruptDispatchError, InterruptRuntime, SoftwareEnv,
};
use std::{
    convert::Infallible,
    future::pending,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::{Notify, watch};

struct Notice;
impl InterruptEnvelope for Notice {
    type Vector = ();
    fn vector(&self) {}
}

struct Source(watch::Sender<bool>);
impl InterruptSource for Source {
    type Notification = Notice;
    type Ready = ();
    type Error = watch::error::RecvError;
    async fn wait_ready(&self) -> Result<(), Self::Error> {
        self.0.subscribe().wait_for(|ready| *ready).await?;
        Ok(())
    }
    async fn claim(&mut self, (): ()) -> Result<Option<Notice>, Self::Error> {
        Ok(self.0.send_replace(false).then_some(Notice))
    }
}

struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct FailingHandler;
impl InterruptHandler<Notice> for FailingHandler {
    type Error = EnvContextError;
    async fn handle(&mut self, _: &Notice) -> Result<(), Self::Error> {
        Err(EnvContextError::StateBusy)
    }
}

#[tokio::test]
async fn handler_failure_returns_its_cause_and_drops_waiting_program() {
    // Given: a software program waiting for a handler that will fail.
    let drops = Arc::new(AtomicUsize::new(0));
    let (irq, _) = watch::channel(false);
    let runtime = InterruptRuntime::new(Arc::new(SoftwareEnv::new()), Source(irq.clone()));
    // When: the registered handler fails after software suspends.
    let result = runtime
        .run_main(async |interrupts| {
            let _program = Dropped(Arc::clone(&drops));
            interrupts.register((), FailingHandler).expect("register");
            irq.send_replace(true);
            pending::<()>().await;
        })
        .await;
    // Then: runtime returns the original error without waiting for software completion.
    assert!(matches!(
        result,
        Err(InterruptDispatchError::Handler {
            vector: (),
            source: EnvContextError::StateBusy,
        })
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

struct PendingHandler {
    started: Arc<Notify>,
    _dropped: Dropped,
}
impl InterruptHandler<Notice> for PendingHandler {
    type Error = Infallible;
    async fn handle(&mut self, _: &Notice) -> Result<(), Infallible> {
        self.started.notify_one();
        pending().await
    }
}

#[tokio::test]
async fn cancelling_runtime_drops_program_handler_and_releases_env() {
    // Given: an ISR that suspends indefinitely while owning the environment.
    let env = Arc::new(SoftwareEnv::new());
    let drops = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let (irq, _) = watch::channel(false);
    let runtime = InterruptRuntime::new(Arc::clone(&env), Source(irq.clone()));
    // When: the owner drops the complete runtime future after the handler starts.
    {
        let running = runtime.run_main(async |interrupts| {
            let _program = Dropped(Arc::clone(&drops));
            interrupts
                .register(
                    (),
                    PendingHandler {
                        started: Arc::clone(&started),
                        _dropped: Dropped(Arc::clone(&drops)),
                    },
                )
                .expect("register");
            irq.send_replace(true);
            pending::<()>().await;
        });
        tokio::pin!(running);
        tokio::select! {
            () = started.notified() => {},
            result = &mut running => panic!("runtime unexpectedly finished: {result:?}"),
        }
    }
    // Then: neither software nor handler survives, and the environment can run again.
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(env.run_main(async { 7 }).await, 7);
}
