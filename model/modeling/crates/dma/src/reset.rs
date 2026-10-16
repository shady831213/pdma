use crate::Dma;
use pdma_model_contract::dma::{DmaControlValues, ResetValues};

impl<M, SB> Dma<M, SB> {
    pub(crate) fn finish_device_reset(&mut self) {
        if self.resetting()
            && self
                .channels
                .iter()
                .all(|channel| channel.resident.is_none())
        {
            self.active = None;
            self.control.values = DmaControlValues::default();
            self.interrupt = false;
        }
    }
    pub(crate) fn request_reset(&mut self) {
        if !self.resetting() {
            self.control.values.reset = ResetValues::new(1);
            for channel in &mut self.channels {
                channel.request_reset();
            }
            self.release_invalid_grant();
            self.finish_device_reset();
        }
    }
}
