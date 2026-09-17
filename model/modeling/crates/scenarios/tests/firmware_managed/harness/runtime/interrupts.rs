use hardware_interface_runtime::{InterruptEnvelope, InterruptSource, PendingInterrupt};
use pdma_model_host::runtime::Irq;
use pdma_model_software::control::ControlIrq;

/// Independent destinations on the Host CPU.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HostVector {
    /// DMA completion.
    Dma,
    /// Management response doorbell.
    Control,
}

/// Platform notification delivered to Host software.
#[derive(Clone, Debug)]
pub enum HostIrq {
    /// Existing DMA notification.
    Dma(Irq),
    /// Control response notification.
    Control(ControlIrq),
}

impl InterruptEnvelope for HostIrq {
    type Vector = HostVector;

    fn vector(&self) -> HostVector {
        match self {
            Self::Dma(_) => HostVector::Dma,
            Self::Control(_) => HostVector::Control,
        }
    }
}

/// Two independent pending destinations sharing the canonical Host dispatcher.
#[derive(Debug)]
pub struct HostInterrupts {
    pub(super) dma: PendingInterrupt<HostIrq>,
    pub(super) control: PendingInterrupt<HostIrq>,
    pub(super) next: HostVector,
}

impl InterruptSource for HostInterrupts {
    type Notification = HostIrq;
    type Ready = HostVector;
    type Error = tokio::sync::watch::error::RecvError;

    async fn wait_ready(&self) -> Result<HostVector, Self::Error> {
        let (first, second) = match self.next {
            HostVector::Dma => (&self.dma, &self.control),
            HostVector::Control => (&self.control, &self.dma),
        };
        tokio::select! {
            biased;
            ready = first.wait_ready() => {
                ready?;
                Ok(self.next)
            },
            ready = second.wait_ready() => {
                ready?;
                Ok(match self.next {
                    HostVector::Dma => HostVector::Control,
                    HostVector::Control => HostVector::Dma,
                })
            },
        }
    }

    async fn claim(&mut self, ready: HostVector) -> Result<Option<HostIrq>, Self::Error> {
        match ready {
            HostVector::Dma => {
                self.next = HostVector::Control;
                self.dma.claim(()).await
            }
            HostVector::Control => {
                self.next = HostVector::Dma;
                self.control.claim(()).await
            }
        }
    }
}
