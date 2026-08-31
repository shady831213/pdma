use crate::DriverResult;
use pdma_model_contract::semantic::IrqNotification;
use std::{future::Future, pin::Pin};

pub(crate) type IrqHandlerFuture<'a> = Pin<Box<dyn Future<Output = DriverResult<()>> + Send + 'a>>;

pub(crate) trait IrqHandler: Send {
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_>;
}

pub(crate) trait StatefulIrqHandler: Send {
    fn handle(
        &mut self,
        notification: IrqNotification,
    ) -> impl Future<Output = DriverResult<()>> + Send;
}

struct FunctionIrqHandler<F>(F);
struct StatefulIrqHandlerAdapter<H>(H);

pub(crate) const fn irq_handler<F, Fut>(handler: F) -> impl IrqHandler
where
    F: FnMut(IrqNotification) -> Fut + Send,
    Fut: Future<Output = DriverResult<()>> + Send + 'static,
{
    FunctionIrqHandler(handler)
}

pub(crate) const fn stateful_irq_handler<H>(handler: H) -> impl IrqHandler
where
    H: StatefulIrqHandler,
{
    StatefulIrqHandlerAdapter(handler)
}

impl<F, Fut> IrqHandler for FunctionIrqHandler<F>
where
    F: FnMut(IrqNotification) -> Fut + Send,
    Fut: Future<Output = DriverResult<()>> + Send + 'static,
{
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin((self.0)(notification))
    }
}

impl<H: StatefulIrqHandler> IrqHandler for StatefulIrqHandlerAdapter<H> {
    fn handle(&mut self, notification: IrqNotification) -> IrqHandlerFuture<'_> {
        Box::pin(StatefulIrqHandler::handle(&mut self.0, notification))
    }
}
