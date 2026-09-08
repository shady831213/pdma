use crate::{DeviceCore, PdmaMemoryProvisioning};
use pdma_model_contract::abi::registers::{QueueRegisters, irq_offset, locate, queue_offset};
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
    QueueRingLayouts, RingCursor, RingLayout, U32WordValues,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct TestQueue {
    pub(super) index: u32,
    pub(super) layouts: QueueRingLayouts,
}

impl TestQueue {
    pub(super) fn queue_id(self) -> pdma_model_contract::semantic::QueueId {
        pdma_model_contract::semantic::QueueId::new(
            usize::try_from(self.index).expect("queue index"),
        )
    }

    pub(super) fn completion_ring_id(self) -> pdma_model_contract::semantic::CompletionRingId {
        pdma_model_contract::semantic::CompletionRingId::new(self.queue_id().raw())
    }
}

pub(super) fn configure(
    core: &mut DeviceCore,
    provisioning: &mut PdmaMemoryProvisioning,
    index: u32,
    depths: [u32; 3],
) -> TestQueue {
    let [sq, cq, eq] = depths;
    let regions = [
        provisioning
            .allocate(&mut core.memory, u64::from(sq) * 64)
            .expect("SQ allocation"),
        provisioning
            .allocate(&mut core.memory, u64::from(cq) * 32)
            .expect("CQ allocation"),
        provisioning
            .allocate(&mut core.memory, u64::from(eq) * 64)
            .expect("EQ allocation"),
    ];
    let [submission, completion, event] = regions;
    let queue = TestQueue {
        index,
        layouts: QueueRingLayouts::new(
            RingLayout::try_new(submission.region(), u64::from(sq)).expect("SQ layout"),
            RingLayout::try_new(completion.region(), u64::from(cq)).expect("CQ layout"),
            RingLayout::try_new(event.region(), u64::from(eq)).expect("EQ layout"),
        ),
    };
    for (offset, value) in [
        (QueueRegisters::SQ_BASE.offset(), submission.base().raw()),
        (QueueRegisters::SQ_DEPTH.offset(), u64::from(sq)),
        (QueueRegisters::CQ_BASE.offset(), completion.base().raw()),
        (QueueRegisters::CQ_DEPTH.offset(), u64::from(cq)),
        (QueueRegisters::EQ_BASE.offset(), event.base().raw()),
        (QueueRegisters::EQ_DEPTH.offset(), u64::from(eq)),
        (QueueRegisters::ENABLE.offset(), 1),
    ] {
        write_queue(core, queue, offset, value);
    }
    queue
}

pub(super) fn write_queue(core: &mut DeviceCore, queue: TestQueue, local: usize, value: u64) {
    let register =
        locate(queue_offset(queue.index, local).expect("queue offset")).expect("register");
    core.write_mmio_register(register, value)
        .expect("queue write");
}

pub(super) fn read_queue(core: &DeviceCore, queue: TestQueue, local: usize) -> u64 {
    let register =
        locate(queue_offset(queue.index, local).expect("queue offset")).expect("register");
    let bytes = core.read_register(register).expect("queue read");
    let mut word = [0; 8];
    word.get_mut(..bytes.len())
        .expect("register width")
        .copy_from_slice(&bytes);
    u64::from_le_bytes(word)
}

pub(super) fn irq_write(
    core: &mut DeviceCore,
    queue: TestQueue,
    source: u32,
    local: usize,
    value: u64,
) {
    let register =
        locate(irq_offset(queue.index, source, local).expect("source offset")).expect("register");
    core.write_mmio_register(register, value)
        .expect("IRQ write");
}

pub(super) fn submit(core: &mut DeviceCore, queue: TestQueue, command: AbiCommand) {
    let producer = read_queue(core, queue, QueueRegisters::SQ_PRODUCER.offset());
    let address = queue
        .layouts
        .submission()
        .entry_address(RingCursor::new(producer))
        .expect("SQ slot");
    let mut bytes = [0; AbiCommand::RECORD_SIZE];
    command.encode(&mut bytes).expect("descriptor");
    core.memory
        .write_memory(address, &bytes)
        .expect("descriptor DMA");
    write_queue(
        core,
        queue,
        QueueRegisters::SQ_PRODUCER.offset(),
        producer + 1,
    );
}

pub(super) fn doorbell(core: &mut DeviceCore, queue: TestQueue) {
    let producer = read_queue(core, queue, QueueRegisters::SQ_PRODUCER.offset());
    write_queue(core, queue, QueueRegisters::DOORBELL.offset(), producer);
}

pub(super) fn poll_completion(core: &mut DeviceCore, queue: TestQueue) -> Option<AbiCompletion> {
    let consumer = read_queue(core, queue, QueueRegisters::CQ_CONSUMER.offset());
    if consumer == read_queue(core, queue, QueueRegisters::CQ_PRODUCER.offset()) {
        return None;
    }
    let address = queue
        .layouts
        .completion()
        .entry_address(RingCursor::new(consumer))
        .expect("CQ slot");
    let bytes = core
        .memory
        .read_memory(
            address,
            u64::try_from(AbiCompletion::RECORD_SIZE).expect("record size"),
        )
        .expect("CQ read");
    let record = AbiCompletion::decode(&bytes).expect("completion");
    write_queue(
        core,
        queue,
        QueueRegisters::CQ_CONSUMER.offset(),
        consumer + 1,
    );
    Some(record)
}

pub(super) fn poll_event(core: &mut DeviceCore, queue: TestQueue) -> Option<AbiEvent> {
    let consumer = read_queue(core, queue, QueueRegisters::EQ_CONSUMER.offset());
    if consumer == read_queue(core, queue, QueueRegisters::EQ_PRODUCER.offset()) {
        return None;
    }
    let address = queue
        .layouts
        .event()
        .entry_address(RingCursor::new(consumer))
        .expect("EQ slot");
    let bytes = core
        .memory
        .read_memory(
            address,
            u64::try_from(AbiEvent::RECORD_SIZE).expect("record size"),
        )
        .expect("EQ read");
    let record = AbiEvent::decode(&bytes).expect("event");
    write_queue(
        core,
        queue,
        QueueRegisters::EQ_CONSUMER.offset(),
        consumer + 1,
    );
    Some(record)
}

pub(super) const fn copy(id: u64, source: u64, destination: u64, length: u32) -> AbiCommand {
    AbiCommand::from_payload(
        id,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: source,
            destination_dma_address: destination,
            length: U32WordValues { value: length },
        }),
    )
}
