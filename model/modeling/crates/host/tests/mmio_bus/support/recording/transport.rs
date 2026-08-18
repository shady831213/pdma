use super::{Access, RecordingStage, RecordingTransport};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::abi::mmio::{MmioAccessError, MmioByteCount};
use pdma_model_contract::bridge::{PdmaMemoryRegion, QueueActivation};
use pdma_model_contract::semantic::{BufferId, IrqNotification};
use pdma_model_device::{MmioBatchError, PdmaMemoryError, QueueResponseBytes};
use pdma_model_host::ModelResult;
use pdma_model_host::{BusDmaStage, BusSubmission, MmioDeviceTransport};
use std::sync::atomic::Ordering;

impl BusDmaStage for RecordingStage {
    fn proposed_regions(&self) -> &[PdmaMemoryRegion] {
        &self.0
    }
}

impl MmioDeviceTransport for RecordingTransport {
    type Stage = RecordingStage;

    async fn allocate_memory_region(&self, _len: usize) -> ModelResult<PdmaMemoryRegion> {
        Err(pdma_model_host::ModelError::DmaAllocationExhausted)
    }

    async fn stage_memory_regions(&self, lengths: &[u64]) -> Result<Self::Stage, PdmaMemoryError> {
        let bases = [0x1_0000, 0x2_0000, 0x3_0000];
        let regions = lengths
            .iter()
            .copied()
            .zip(bases)
            .enumerate()
            .map(|(index, (length, base))| {
                PdmaMemoryRegion::try_new(BufferId::new(index), DmaAddress::new(base), length)
                    .map_err(|_| PdmaMemoryError::InvalidRegion)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RecordingStage(regions))
    }

    async fn read_dma(
        &self,
        _address: DmaAddress,
        _length: u64,
    ) -> Result<Vec<u8>, PdmaMemoryError> {
        Err(PdmaMemoryError::Unmapped)
    }

    async fn write_dma(&self, _address: DmaAddress, _bytes: &[u8]) -> Result<(), PdmaMemoryError> {
        Err(PdmaMemoryError::Unmapped)
    }

    async fn bootstrap_batch(
        &self,
        _stage: Self::Stage,
        _activation: QueueActivation,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        self.record(Access::BootstrapBatch);
        if self.batch_failure.swap(false, Ordering::SeqCst) {
            return Err(MmioBatchError::InvalidShape);
        }
        Ok([0; 72])
    }

    async fn submission_batch(
        &self,
        _request: BusSubmission<Self::Stage>,
    ) -> Result<QueueResponseBytes, MmioBatchError> {
        self.record(Access::SubmissionBatch);
        Ok([0; 72])
    }

    async fn read_mmio(
        &self,
        offset: u32,
        width: MmioByteCount,
    ) -> Result<Vec<u8>, MmioAccessError> {
        self.record(Access::Read(offset, width));
        let value = match offset {
            0x16c => u64::from(self.queue_result.load(Ordering::SeqCst).max(1)),
            0x224 => {
                if self.irq_command.load(Ordering::SeqCst) != 9
                    || self.claimed.lock().is_ok_and(|claimed| claimed.is_some())
                {
                    1
                } else {
                    2
                }
            }
            0x254 => 0x13,
            0x258 => 1,
            0x25c => 2,
            0x260 => self
                .claimed
                .lock()
                .ok()
                .and_then(|value| *value)
                .map_or(0, |value| u64::try_from(value.vector().raw()).unwrap_or(0)),
            0x268 => self
                .claimed
                .lock()
                .ok()
                .and_then(|value| *value)
                .and_then(|value| match value {
                    IrqNotification::Message(message) => u64::try_from(message.id().raw()).ok(),
                    IrqNotification::Line(_) => None,
                })
                .unwrap_or(0),
            _ => 0,
        };
        match width {
            4 => Ok(u32::try_from(value)
                .map_or([0; 4], u32::to_le_bytes)
                .to_vec()),
            8 => Ok(value.to_le_bytes().to_vec()),
            _ => Err(MmioAccessError::WrongWidth {
                offset,
                expected: 0,
                actual: width,
            }),
        }
    }

    async fn write_mmio(&self, offset: u32, bytes: &[u8]) -> Result<(), MmioAccessError> {
        self.record(Access::Write(offset, bytes.to_vec()));
        if self
            .fail_write
            .lock()
            .is_ok_and(|mut value| value.take() == Some(offset))
        {
            return Err(MmioAccessError::UnmappedOffset { offset });
        }
        if offset == 0x100 && self.gate_queue_start.swap(false, Ordering::SeqCst) {
            self.queue_started.notify_one();
            self.queue_release.notified().await;
        }
        if offset == 0x220 && bytes == 9_u32.to_le_bytes() {
            let notification = if self
                .empty_claims
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                None
            } else {
                self.notification
                    .lock()
                    .ok()
                    .and_then(|mut value| value.take())
            };
            if let Ok(mut claimed) = self.claimed.lock() {
                *claimed = notification;
            }
        }
        if offset == 0x220 {
            if let Ok(command) = <[u8; 4]>::try_from(bytes) {
                self.irq_command
                    .store(u32::from_le_bytes(command), Ordering::SeqCst);
            }
        }
        Ok(())
    }

    async fn wait_irq_notification_ready(&self) -> bool {
        self.notification.lock().is_ok_and(|value| value.is_some())
    }
}
use crate::register::legacy_integration_tests::pdma_model_host;
