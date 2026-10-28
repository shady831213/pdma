//! A software program registers an ISR and waits for its notification.
//!
//! Run: `cargo run -p hw-model-swenv --example first_program`.
//! Assembly connects the interrupt source. Software owns handler registration;
//! the runtime owns main/ISR scheduling and the lifetime of dispatch.

use hw_model_irq::{InterruptEnvelope, InterruptHandler, interrupt_wire};
use hw_model_swenv::{
    InterruptRegistrationError, InterruptRuntime, NotificationWait, Notifier, SoftwareEnv,
    notifications,
};
use std::{convert::Infallible, error::Error, sync::Arc};

#[derive(Clone)]
struct Notice;

impl InterruptEnvelope for Notice {
    type Vector = u8;

    fn vector(&self) -> u8 {
        1
    }
}

struct Handler(Notifier);

impl InterruptHandler<Notice> for Handler {
    type Error = Infallible;

    async fn handle(&mut self, _: &Notice) -> Result<(), Self::Error> {
        // The ISR wakes main rather than waiting for main to run during this turn.
        self.0.notify();
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let (emitter, receiver) = interrupt_wire(Notice);
    let (done, completed) = notifications();
    let runtime = InterruptRuntime::new(Arc::new(SoftwareEnv::new()), receiver);
    let (program, summary) = runtime
        .run_main(async |isrs| {
            isrs.register(1, Handler(done))?;
            // A toy hardware stimulus, after the program installs its handler.
            emitter.raise();
            // Pending releases main's turn so the runtime can dispatch the ISR.
            completed.wait_notification().await;
            Ok::<_, InterruptRegistrationError<u8>>(())
        })
        .await?;
    // Dispatch errors and the program's own errors are separate result layers.
    program?;
    assert_eq!(summary.deliveries(), 1);
    println!("program observed one ISR; scoped dispatch finished");
    Ok(())
}
