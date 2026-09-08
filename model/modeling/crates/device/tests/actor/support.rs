#![allow(
    clippy::expect_used,
    reason = "test fixtures fail immediately with contextual setup and observation errors"
)]

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    abi::{
        AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
        DmaAddress,
        registers::{IrqSourceRegisters, QueueRegisters, irq_offset, queue_offset},
    },
    semantic::ResourceConfig,
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, MmioRegisterEndpoint};

pub(super) struct Fixture {
    pub(super) device: AsyncDevice,
    pub(super) handle: AsyncDeviceHandle,
    pub(super) endpoint: MmioRegisterEndpoint,
    pub(super) sq: DmaAddress,
    pub(super) cq: DmaAddress,
    pub(super) eq: DmaAddress,
    pub(super) source: DmaAddress,
    pub(super) destination: DmaAddress,
}

impl Fixture {
    pub(super) async fn new(config: ResourceConfig) -> (Self, AsyncDeviceWorker) {
        let (device, worker) = AsyncDevice::new(
            pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
                hardware_interface_runtime::SharedMemory::new(),
            )),
            config,
        );
        let handle = device.handle();
        let endpoint = handle.register_endpoint();
        let sq = handle.allocate_memory_region(256).await.expect("SQ").base();
        let cq = handle.allocate_memory_region(128).await.expect("CQ").base();
        let eq = handle.allocate_memory_region(256).await.expect("EQ").base();
        let source = handle
            .allocate_memory_region(8)
            .await
            .expect("source")
            .base();
        let destination = handle
            .allocate_memory_region(8)
            .await
            .expect("destination")
            .base();
        let fixture = Self {
            device,
            handle,
            endpoint,
            sq,
            cq,
            eq,
            source,
            destination,
        };
        for (local, address) in [
            (QueueRegisters::SQ_BASE.offset(), sq),
            (QueueRegisters::CQ_BASE.offset(), cq),
            (QueueRegisters::EQ_BASE.offset(), eq),
        ] {
            fixture
                .write_queue(local, &address.raw().to_le_bytes())
                .await;
        }
        for local in [
            QueueRegisters::SQ_DEPTH.offset(),
            QueueRegisters::CQ_DEPTH.offset(),
            QueueRegisters::EQ_DEPTH.offset(),
        ] {
            fixture.write_queue(local, &4_u32.to_le_bytes()).await;
        }
        fixture
            .write_queue(QueueRegisters::ENABLE.offset(), &1_u32.to_le_bytes())
            .await;
        (fixture, worker)
    }

    pub(super) async fn write_queue(&self, local: usize, bytes: &[u8]) {
        self.endpoint
            .write(queue_offset(0, local).expect("queue address"), bytes)
            .await
            .expect("queue write");
    }

    pub(super) async fn cursor(&self, local: usize) -> u64 {
        let mut bytes = [0; 8];
        self.endpoint
            .read(queue_offset(0, local).expect("queue address"), &mut bytes)
            .await
            .expect("queue read");
        u64::from_le_bytes(bytes)
    }

    pub(super) async fn copy(&self, slot: u64, payload: AbiCommandPayloadCopyValues) {
        let command = AbiCommand::from_payload(slot, AbiCommandPayloadValues::Copy(payload));
        let mut bytes = [0; 64];
        command.encode(&mut bytes).expect("descriptor");
        self.handle
            .write_memory(DmaAddress::new(self.sq.raw() + slot * 64), &bytes)
            .await
            .expect("SQ write");
    }

    pub(super) fn payload(&self, length: u32) -> AbiCommandPayloadCopyValues {
        AbiCommandPayloadCopyValues {
            source_dma_address: self.source.raw(),
            destination_dma_address: self.destination.raw(),
            length: length.into(),
        }
    }

    pub(super) async fn publish(&self, producer: u64) {
        self.write_queue(
            QueueRegisters::SQ_PRODUCER.offset(),
            &producer.to_le_bytes(),
        )
        .await;
        self.write_queue(QueueRegisters::DOORBELL.offset(), &producer.to_le_bytes())
            .await;
    }

    pub(super) async fn completion(&self, slot: u64) -> AbiCompletion {
        let bytes = self
            .handle
            .read_memory(DmaAddress::new(self.cq.raw() + slot * 32), 32)
            .await
            .expect("CQ bytes");
        AbiCompletion::decode(&bytes).expect("completion")
    }

    pub(super) async fn event(&self, slot: u64) -> AbiEvent {
        let bytes = self
            .handle
            .read_memory(DmaAddress::new(self.eq.raw() + slot * 64), 64)
            .await
            .expect("EQ bytes");
        AbiEvent::decode(&bytes).expect("event")
    }

    pub(super) async fn enable_irq(&self, source: u32) {
        for (local, value) in [
            (IrqSourceRegisters::ENABLED.offset(), 1_u32),
            (IrqSourceRegisters::MASKED.offset(), 0),
        ] {
            self.endpoint
                .write(
                    irq_offset(0, source, local).expect("IRQ address"),
                    &value.to_le_bytes(),
                )
                .await
                .expect("IRQ write");
        }
    }
}
