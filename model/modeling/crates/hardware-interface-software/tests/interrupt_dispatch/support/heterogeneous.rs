use super::{Notice, ToyError, ToySource, freeze, noop};
use hardware_interface_irq::InterruptHandler;
use hardware_interface_software::{HandlerRegistry, InterruptDispatcher, SoftwareEnv};
use std::sync::{Arc, Mutex};

struct First(Arc<Mutex<Vec<&'static str>>>);
struct Second(Arc<Mutex<Vec<&'static str>>>);

impl InterruptHandler<Notice> for First {
    type Error = ToyError;

    async fn handle(&mut self, _notification: &Notice) -> Result<(), Self::Error> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("first");
        Ok(())
    }
}

impl InterruptHandler<Notice> for Second {
    type Error = ToyError;

    async fn handle(&mut self, _notification: &Notice) -> Result<(), Self::Error> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("second");
        Ok(())
    }
}

enum TypedHandlers {
    First(First),
    Second(Second),
}

impl InterruptHandler<Notice> for TypedHandlers {
    type Error = ToyError;

    async fn handle(&mut self, notification: &Notice) -> Result<(), Self::Error> {
        match self {
            Self::First(handler) => handler.handle(notification).await,
            Self::Second(handler) => handler.handle(notification).await,
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn typed_enum_supports_heterogeneous_handlers_without_erasure() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut handlers = HandlerRegistry::new();
    assert!(
        handlers
            .register(1, TypedHandlers::First(First(Arc::clone(&observed))))
            .is_ok()
    );
    assert!(
        handlers
            .register(2, TypedHandlers::Second(Second(Arc::clone(&observed))))
            .is_ok()
    );
    let source = ToySource::new([
        Ok(Some(Notice {
            vector: 1,
            value: 1,
        })),
        Ok(Some(Notice {
            vector: 2,
            value: 2,
        })),
    ]);
    let task = InterruptDispatcher::start(
        Arc::new(SoftwareEnv::new()),
        freeze(handlers),
        source,
        noop(),
    );
    while observed
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len()
        != 2
    {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        task.shutdown_and_join().await.expect("joins").deliveries(),
        2
    );
    assert_eq!(
        *observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ["first", "second"]
    );
}
