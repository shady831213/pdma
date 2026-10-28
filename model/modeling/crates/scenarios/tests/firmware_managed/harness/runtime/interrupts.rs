use hw_model_irq::{InterruptEnvelope, InterruptReceiver, InterruptSource};
use pdma_model_host::runtime::Irq;
use pdma_model_software::control::ControlIrq;

/// Independent destinations on the Host CPU.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HostVector {
    /// DMA completion.
    Dma,
    /// Management response doorbell.
    Control,
    /// Firmware event doorbell.
    Event,
}

/// Platform notification delivered to Host software.
#[derive(Clone, Debug)]
pub enum HostIrq {
    /// Existing DMA notification.
    Dma(Irq),
    /// Control response notification.
    Control(ControlIrq),
    /// Firmware asynchronous event notification.
    Event(ControlIrq),
}

impl InterruptEnvelope for HostIrq {
    type Vector = HostVector;

    fn vector(&self) -> HostVector {
        match self {
            Self::Dma(_) => HostVector::Dma,
            Self::Control(_) => HostVector::Control,
            Self::Event(_) => HostVector::Event,
        }
    }
}

/// Independent DMA, response and event pending destinations sharing the canonical Host dispatcher.
#[derive(Debug)]
pub struct HostInterrupts {
    pub(super) dma: InterruptReceiver<HostIrq>,
    pub(super) control: Option<InterruptReceiver<HostIrq>>,
    pub(super) event: Option<InterruptReceiver<HostIrq>>,
    pub(super) next: HostVector,
}

impl InterruptSource for HostInterrupts {
    type Notification = HostIrq;
    type Ready = HostVector;
    type Error = tokio::sync::watch::error::RecvError;

    async fn wait_ready(&self) -> Result<HostVector, Self::Error> {
        let mut sources = [
            (HostVector::Dma, Some(&self.dma)),
            (HostVector::Control, self.control.as_ref()),
            (HostVector::Event, self.event.as_ref()),
        ];
        sources.rotate_left(match self.next {
            HostVector::Dma => 0,
            HostVector::Control => 1,
            HostVector::Event => 2,
        });
        let [first, second, third] = sources;
        tokio::select! {
            biased;
            result = wait(first) => result,
            result = wait(second) => result,
            result = wait(third) => result,
        }
    }

    async fn claim(&mut self, ready: HostVector) -> Result<Option<HostIrq>, Self::Error> {
        let source = match ready {
            HostVector::Dma => {
                self.next = HostVector::Control;
                Some(&mut self.dma)
            }
            HostVector::Control => {
                self.next = HostVector::Event;
                self.control.as_mut()
            }
            HostVector::Event => {
                self.next = HostVector::Dma;
                self.event.as_mut()
            }
        };
        match source {
            Some(source) => source.claim(()).await,
            None => Ok(None),
        }
    }
}

async fn wait(
    (vector, source): (HostVector, Option<&InterruptReceiver<HostIrq>>),
) -> Result<HostVector, tokio::sync::watch::error::RecvError> {
    match source {
        Some(source) => {
            source.wait_ready().await?;
            Ok(vector)
        }
        None => std::future::pending().await,
    }
}
