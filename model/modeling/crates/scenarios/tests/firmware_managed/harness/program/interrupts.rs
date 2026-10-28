use super::transfer;
use crate::harness::runtime::{AccessError, HostIrq};
use hw_model_irq::InterruptHandler;
use pdma_model_host::DmaError;
use pdma_model_software::control::ControlIsr;

pub(in crate::harness) enum Handler {
    Wake(tokio::sync::mpsc::UnboundedSender<()>),
    Dma(Box<transfer::Handler>),
    Control(ControlIsr),
}

impl InterruptHandler<HostIrq> for Handler {
    type Error = HandlerError;

    async fn handle(&mut self, irq: &HostIrq) -> Result<(), Self::Error> {
        match (self, irq) {
            (Self::Wake(sender), HostIrq::Dma(_)) => {
                let _closed = sender.send(());
                Ok(())
            }
            (Self::Dma(handler), HostIrq::Dma(irq)) => {
                handler.handle(irq).await.map_err(HandlerError::Dma)
            }
            (Self::Control(handler), HostIrq::Control(irq) | HostIrq::Event(irq)) => {
                match handler.handle(irq).await {
                    Ok(()) => Ok(()),
                    Err(never) => match never {},
                }
            }
            (Self::Dma(_) | Self::Wake(_), HostIrq::Control(_) | HostIrq::Event(_))
            | (Self::Control(_), HostIrq::Dma(_)) => Err(HandlerError::VectorMismatch),
        }
    }
}

#[derive(Debug)]
pub(in crate::harness) enum HandlerError {
    Dma(DmaError<AccessError>),
    VectorMismatch,
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dma(error) => error.fmt(f),
            Self::VectorMismatch => f.write_str("handler registered on incompatible vector"),
        }
    }
}

impl std::error::Error for HandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dma(error) => Some(error),
            Self::VectorMismatch => None,
        }
    }
}
