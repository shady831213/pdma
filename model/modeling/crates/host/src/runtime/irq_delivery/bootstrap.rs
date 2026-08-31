use super::{
    HasHandlers, IrqDelivery, IrqDeliveryHandle, IrqDeliverySummary, IrqHandler,
    IrqHandlerRegistry, NoHandler,
};
use crate::DriverPort;
use crate::runtime::interrupt::{InterruptError, InterruptResult};
use pdma_model_contract::semantic::IrqVector;
use tokio::sync::oneshot;

type RunnableDelivery<P> = IrqDelivery<P, HasHandlers<'static>>;

/// Software-side type-state builder for a deferred host IRQ delivery actor.
#[derive(Debug)]
pub(crate) struct IrqDeliveryBootstrap<P> {
    delivery: IrqDelivery<P, NoHandler>,
    start: oneshot::Sender<RunnableDelivery<P>>,
}

/// Harness-side runner that waits for software to finish handler registration.
#[derive(Debug)]
pub(crate) struct IrqDeliveryRunner<P> {
    start: oneshot::Receiver<RunnableDelivery<P>>,
}

impl<P> IrqDelivery<P, NoHandler> {
    /// Splits software registration from harness-owned actor execution.
    pub(crate) fn defer(self) -> (IrqDeliveryBootstrap<P>, IrqDeliveryRunner<P>) {
        let (start, registered) = oneshot::channel();
        (
            IrqDeliveryBootstrap {
                delivery: self,
                start,
            },
            IrqDeliveryRunner { start: registered },
        )
    }
}

impl<P> IrqDeliveryBootstrap<P> {
    /// Registers one vector handler and publishes the fixed registry to the runner.
    pub(crate) fn register_handler<H>(
        self,
        vector: IrqVector,
        handler: H,
    ) -> InterruptResult<IrqDeliveryHandle>
    where
        H: IrqHandler + 'static,
    {
        self.register_handlers(move |registry| registry.register(vector, handler))
    }

    /// Registers all vector handlers atomically, then publishes the actor to the runner.
    pub(crate) fn register_handlers<F>(self, configure: F) -> InterruptResult<IrqDeliveryHandle>
    where
        F: FnOnce(&mut IrqHandlerRegistry<'static>) -> InterruptResult<()>,
    {
        let mut registry = IrqHandlerRegistry::new();
        configure(&mut registry)?;
        let handler = registry.freeze()?;
        let (delivery, handle) = self.delivery.with_handlers(handler);
        self.start
            .send(delivery)
            .map_err(|_| InterruptError::DeliveryUnavailable)?;
        Ok(handle)
    }
}

impl<P> IrqDeliveryRunner<P>
where
    P: DriverPort + 'static,
{
    /// Waits for software registration, then runs the host delivery actor.
    pub(crate) async fn run(self) -> InterruptResult<IrqDeliverySummary> {
        let delivery = self
            .start
            .await
            .map_err(|_| InterruptError::DeliveryUnavailable)?;
        delivery.run().await
    }
}
