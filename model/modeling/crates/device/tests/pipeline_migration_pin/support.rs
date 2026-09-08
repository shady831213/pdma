use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{QueueRegisters, RegisterLocation, locate, queue_offset};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
    DmaAddress, RingLayout,
};
use pdma_model_contract::semantic::{ResourceConfig, TraceEvent, TraceEventKind};
use pdma_model_device::{
    AsyncDevice, AsyncDeviceHandle, AsyncDeviceWorker, AsyncStepStatus, MmioRegisterEndpoint,
};

pub(crate) struct Fixture {
    pub(crate) handle: AsyncDeviceHandle,
    pub(crate) endpoint: MmioRegisterEndpoint,
    worker: Option<AsyncDeviceWorker>,
    rings: [DmaAddress; 3],
}

impl Fixture {
    pub(crate) async fn new(resources: ResourceConfig) -> Self {
        let (device, worker) = AsyncDevice::new(
            pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
                hardware_interface_runtime::SharedMemory::new(),
            )),
            resources,
        );
        let handle = device.handle();
        let stage = handle
            .stage_memory_regions(&[
                RingLayout::<AbiCommand>::checked_extent(4).expect("command extent"),
                RingLayout::<AbiCompletion>::checked_extent(1).expect("completion extent"),
                RingLayout::<AbiEvent>::checked_extent(1).expect("event extent"),
            ])
            .await
            .expect("ring stage");
        let rings = [
            stage.regions().first().expect("submission region").base(),
            stage.regions().get(1).expect("completion region").base(),
            stage.regions().last().expect("event region").base(),
        ];
        handle
            .commit_memory_stage(stage)
            .await
            .expect("ring mappings");
        let endpoint = handle.register_endpoint();
        let fixture = Self {
            handle,
            endpoint,
            worker: Some(worker),
            rings,
        };
        fixture.activate(&fixture.endpoint).await;
        fixture
    }

    pub(crate) async fn activate(&self, endpoint: &MmioRegisterEndpoint) {
        for (address, (base, depth, size)) in self.rings.iter().zip([
            (
                QueueRegisters::SQ_BASE.offset(),
                QueueRegisters::SQ_DEPTH.offset(),
                4,
            ),
            (
                QueueRegisters::CQ_BASE.offset(),
                QueueRegisters::CQ_DEPTH.offset(),
                1,
            ),
            (
                QueueRegisters::EQ_BASE.offset(),
                QueueRegisters::EQ_DEPTH.offset(),
                1,
            ),
        ]) {
            register_write(
                endpoint,
                queue_offset(0, base).expect("base offset"),
                address.raw(),
            )
            .await;
            register_write(
                endpoint,
                queue_offset(0, depth).expect("depth offset"),
                size,
            )
            .await;
        }
        register_write(
            endpoint,
            queue_offset(0, QueueRegisters::ENABLE.offset()).expect("enable offset"),
            1,
        )
        .await;
    }

    pub(crate) async fn submit(
        &self,
        producer: u64,
        command: u64,
        source: DmaAddress,
        target: DmaAddress,
    ) {
        let descriptor = AbiCommand::from_payload(
            command,
            AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
                source_dma_address: source.raw(),
                destination_dma_address: target.raw(),
                length: 4.into(),
            }),
        );
        let mut bytes = [0; 64];
        descriptor.encode(&mut bytes).expect("descriptor encode");
        let address = self
            .rings
            .first()
            .expect("submission ring")
            .checked_add(producer * 64)
            .expect("descriptor address");
        write(&self.handle, address, &bytes).await;
        self.cursor(QueueRegisters::SQ_PRODUCER.offset(), producer + 1)
            .await;
    }

    pub(crate) async fn cursor(&self, local: usize, value: u64) {
        register_write(
            &self.endpoint,
            queue_offset(0, local).expect("cursor offset"),
            value,
        )
        .await;
    }

    pub(crate) async fn cursors(&self) -> [u64; 3] {
        let mut values = [0; 3];
        for (value, local) in values.iter_mut().zip([
            QueueRegisters::SQ_CONSUMER.offset(),
            QueueRegisters::CQ_PRODUCER.offset(),
            QueueRegisters::CQ_CONSUMER.offset(),
        ]) {
            *value = register_read(
                &self.endpoint,
                queue_offset(0, local).expect("cursor offset"),
            )
            .await;
        }
        values
    }

    pub(crate) fn start(&mut self) -> tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary> {
        tokio::spawn(self.worker.take().expect("worker starts once").run())
    }
}

fn width(offset: u32) -> usize {
    match locate(offset).expect("register location") {
        RegisterLocation::Global { register } => register.width(),
        RegisterLocation::Queue { register, .. } => register.width(),
        RegisterLocation::Irq { register, .. } => register.width(),
    }
}

pub(crate) async fn register_write(endpoint: &MmioRegisterEndpoint, offset: u32, value: u64) {
    endpoint
        .write(
            offset,
            value
                .to_le_bytes()
                .get(..width(offset))
                .expect("register width"),
        )
        .await
        .expect("register write");
}

pub(crate) async fn register_read(endpoint: &MmioRegisterEndpoint, offset: u32) -> u64 {
    let mut bytes = [0; 8];
    endpoint
        .read(
            offset,
            bytes.get_mut(..width(offset)).expect("register width"),
        )
        .await
        .expect("register read");
    u64::from_le_bytes(bytes)
}

pub(crate) async fn operands(
    handle: &AsyncDeviceHandle,
) -> [pdma_model_device::MappedDmaRegion; 3] {
    let stage = handle
        .stage_memory_regions(&[4; 3])
        .await
        .expect("operand stage");
    let result = [
        *stage.regions().first().expect("source region"),
        *stage.regions().get(1).expect("first target region"),
        *stage.regions().last().expect("second target region"),
    ];
    handle
        .commit_memory_stage(stage)
        .await
        .expect("operand commit");
    result
}

pub(crate) async fn write(handle: &AsyncDeviceHandle, address: DmaAddress, bytes: &[u8]) {
    handle
        .write_memory(address, bytes)
        .await
        .expect("DMA write");
}

pub(crate) async fn read(handle: &AsyncDeviceHandle, address: DmaAddress) -> Vec<u8> {
    handle.read_memory(address, 4).await.expect("DMA read")
}

pub(crate) async fn wait_idle(handle: &AsyncDeviceHandle) {
    let mut snapshot = handle.snapshot().await;
    while snapshot.last_status != AsyncStepStatus::Idle {
        snapshot = handle.wait_for_change_after(snapshot).await;
    }
}

pub(crate) fn count(events: &[TraceEvent], kind: TraceEventKind) -> usize {
    events.iter().filter(|event| event.kind() == kind).count()
}

pub(crate) fn adjacent(
    events: &[TraceEvent],
    first: TraceEventKind,
    second: TraceEventKind,
) -> bool {
    events
        .windows(2)
        .any(|pair| matches!(pair, [left, right] if left.kind() == first && right.kind() == second))
}
