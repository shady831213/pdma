use crate::Error;
use hw_model_io::register::RegisterAccess;
use pdma_model_contract::{
    control::{ControlEvent, ControlEventKind},
    dma::{ChannelId, DmaFatalCode},
    ring::RingError,
};
use pdma_model_software::{control::ManagementAddresses, dma::ChannelControl};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Monitor<'a, B> {
    bus: &'a B,
    addresses: ManagementAddresses,
    count: usize,
    control: ChannelControl,
    failed: AtomicBool,
    report: AtomicBool,
}

impl<'a, B: RegisterAccess + Sync> Monitor<'a, B> {
    pub(super) async fn new<M>(
        bus: &'a B,
        addresses: ManagementAddresses,
        count: usize,
    ) -> Result<Self, Error<B::Error, M>> {
        let control = ChannelControl::discover_at(bus, ChannelId::new(0), addresses.dma)
            .await
            .map_err(Error::Dma)?;
        control
            .set_dma_irq_enabled(bus, true)
            .await
            .map_err(Error::Dma)?;
        Ok(Self {
            bus,
            addresses,
            count,
            control,
            failed: AtomicBool::new(false),
            report: AtomicBool::new(false),
        })
    }

    pub(super) fn failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }

    pub(super) async fn observe<M>(&self) -> Result<(), Error<B::Error, M>> {
        match self.control.dma_fatal(self.bus).await.map_err(Error::Dma)? {
            DmaFatalCode::None => {}
            DmaFatalCode::Internal => {
                if !self.failed.swap(true, Ordering::SeqCst) {
                    self.report.store(true, Ordering::SeqCst);
                }
            }
        }
        if self.failed() {
            self.close().await?;
        }
        Ok(())
    }

    pub(super) async fn healthy<M>(&self) -> Result<(), Error<B::Error, M>> {
        self.observe().await?;
        if self.failed() {
            return Err(Error::DeviceFailed);
        }
        Ok(())
    }

    pub(super) fn take_event(&self) -> Option<ControlEvent> {
        self.report
            .swap(false, Ordering::SeqCst)
            .then_some(ControlEvent {
                kind: ControlEventKind::DmaFatal,
                code: pdma_model_contract::dma::DmaFatalValues::RESET
                    .with_code(DmaFatalCode::Internal)
                    .raw(),
            })
    }

    pub(super) async fn reset<M>(&self) -> Result<(), Error<B::Error, M>> {
        self.close().await?;
        self.control.reset_dma(self.bus).await.map_err(Error::Dma)?;
        self.failed.store(false, Ordering::SeqCst);
        self.control
            .set_dma_irq_enabled(self.bus, true)
            .await
            .map_err(Error::Dma)?;
        self.healthy().await
    }

    async fn close<M>(&self) -> Result<(), Error<B::Error, M>> {
        for index in 0..self.count {
            self.bus
                .write(
                    self.addresses
                        .window_control(ChannelId::new(index))
                        .ok_or(Error::Ring(RingError::Geometry))?,
                    &0_u32.to_le_bytes(),
                )
                .await
                .map_err(Error::Bus)?;
        }
        Ok(())
    }
}
