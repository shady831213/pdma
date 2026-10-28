//! Software registration through an environment-bound runtime, without dispatcher assembly.

use hw_model_irq::{InterruptEnvelope, InterruptHandler, InterruptSource};
use hw_model_swenv::{EnvContextError, EnvLocal, InterruptRegistrationError, SoftwareEnv};
use std::sync::Arc;
use tokio::sync::{oneshot, watch};

#[derive(Clone, Copy)]
struct Notice(u8);

impl InterruptEnvelope for Notice {
    type Vector = u8;
    fn vector(&self) -> u8 {
        self.0
    }
}

#[tokio::test]
async fn rejected_registration_preserves_the_installed_handler() {
    // Given: a runtime and an unrelated environment.
    let env = Arc::new(SoftwareEnv::new());
    let other = SoftwareEnv::new();
    let calls = env.local(0);
    let rejected = env.local(0);
    let (pending, _) = watch::channel(None);
    let runtime = hw_model_swenv::InterruptRuntime::new(Arc::clone(&env), Source(pending.clone()));

    // When: another environment attempts registration, then software registers twice.
    let (count, summary) = runtime
        .run_main(async |interrupts| {
            let wrong_env = other
                .run_main(async {
                    interrupts.register(
                        3,
                        Handler {
                            calls: rejected.clone(),
                            done: None,
                        },
                    )
                })
                .await;
            assert_eq!(
                wrong_env,
                Err(InterruptRegistrationError::Env(EnvContextError::Mismatch))
            );
            let (done, received) = oneshot::channel();
            interrupts
                .register(
                    3,
                    Handler {
                        calls: calls.clone(),
                        done: Some(done),
                    },
                )
                .expect("first handler");
            assert_eq!(
                interrupts.register(
                    3,
                    Handler {
                        calls: rejected.clone(),
                        done: None
                    }
                ),
                Err(InterruptRegistrationError::Duplicate(3))
            );
            pending.send_replace(Some(Notice(3)));
            received.await.expect("original handler")
        })
        .await
        .expect("runtime finishes");

    // Then: only the original handler ran.
    assert_eq!(count, 1);
    assert_eq!(summary.deliveries(), 1);
    assert_eq!(
        env.run_main(async { rejected.with(|count| *count) }).await,
        Ok(0)
    );
}

#[tokio::test]
async fn program_can_finish_without_installing_an_isr() {
    // Given: an idle source and a program that will not need interrupts.
    let (pending, _) = watch::channel(None);
    let runtime =
        hw_model_swenv::InterruptRuntime::new(Arc::new(SoftwareEnv::new()), Source(pending));
    // When: the program returns without registration.
    let (value, summary) = runtime
        .run_main::<Handler, _>(async |_| 42)
        .await
        .expect("finish");
    // Then: runtime stops the waiting dispatcher and retains the program result.
    assert_eq!(value, 42);
    assert_eq!(summary.deliveries(), 0);
}

struct Source(watch::Sender<Option<Notice>>);

impl InterruptSource for Source {
    type Notification = Notice;
    type Ready = ();
    type Error = watch::error::RecvError;

    async fn wait_ready(&self) -> Result<(), Self::Error> {
        let mut pending = self.0.subscribe();
        pending.wait_for(Option::is_some).await?;
        Ok(())
    }

    async fn claim(&mut self, (): ()) -> Result<Option<Notice>, Self::Error> {
        Ok(self.0.send_replace(None))
    }
}

struct Handler {
    calls: EnvLocal<usize>,
    done: Option<oneshot::Sender<usize>>,
}

impl InterruptHandler<Notice> for Handler {
    type Error = hw_model_swenv::EnvContextError;

    async fn handle(&mut self, _: &Notice) -> Result<(), Self::Error> {
        let count = self.calls.with(|count| {
            *count += 1;
            *count
        })?;
        if let Some(done) = self.done.take() {
            assert!(done.send(count).is_ok(), "program awaits handler");
        }
        Ok(())
    }
}

#[tokio::test]
async fn program_registers_after_suspension_and_handler_runs_on_its_env() {
    // Given: an idle interrupt source and state owned by the program environment.
    let env = Arc::new(SoftwareEnv::new());
    let calls = env.local(0);
    let (pending, _) = watch::channel(None);
    let runtime = hw_model_swenv::InterruptRuntime::new(Arc::clone(&env), Source(pending.clone()));

    // When: software yields, registers its ISR and raises the test interrupt.
    let (count, summary) = runtime
        .run_main(async |interrupts| {
            tokio::task::yield_now().await;
            let (done, received) = oneshot::channel();
            interrupts
                .register(
                    3,
                    Handler {
                        calls: calls.clone(),
                        done: Some(done),
                    },
                )
                .expect("software registers ISR");
            pending.send_replace(Some(Notice(3)));
            received.await.expect("ISR completes")
        })
        .await
        .expect("runtime finishes");

    // Then: delivery used the same environment and was accounted before runtime returned.
    assert_eq!(count, 1);
    assert_eq!(summary.deliveries(), 1);
    assert_eq!(
        env.run_main(async { calls.with(|count| *count) }).await,
        Ok(1)
    );
}
