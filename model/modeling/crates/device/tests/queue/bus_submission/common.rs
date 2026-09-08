use crate::queue::hardware::{self, read, write};
use pdma_model_contract::abi::registers::{QueueRegisters, queue_offset};
use pdma_model_contract::abi::{AbiCommand, AbiCompletion, AbiEvent, DmaAddress};
use pdma_model_contract::semantic::{ResourceConfig, RingCursor};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, MappedDmaRegion};

pub(in crate::queue) type TestResult<T = ()> = hardware::TestResult<T>;

pub(in crate::queue) struct Fixture {
    pub(in crate::queue) handle: AsyncDeviceHandle,
    pub(in crate::queue) submission: DmaAddress,
    completion: DmaAddress,
    event: DmaAddress,
    pub(in crate::queue) worker: tokio::task::JoinHandle<pdma_model_device::AsyncRunSummary>,
}

#[derive(Debug, PartialEq)]
pub(in crate::queue) enum Record {
    Completion(AbiCompletion),
    Event(AbiEvent),
}

pub(in crate::queue) struct RunResult {
    pub(in crate::queue) consumer: RingCursor,
    pub(in crate::queue) record: Option<Record>,
    pub(in crate::queue) destinations: Vec<Vec<u8>>,
    pub(in crate::queue) steps: usize,
    pub(in crate::queue) traces: Vec<pdma_model_contract::semantic::TraceEvent>,
    pub(in crate::queue) native_steps: usize,
}

pub(in crate::queue) async fn fixture(resources: ResourceConfig) -> TestResult<Fixture> {
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        resources,
    );
    let handle = runner.handle();
    let [submission, completion, event] = hardware::activate(&handle, 0, [2, 2, 2]).await?;
    Ok(Fixture {
        handle,
        submission,
        completion,
        event,
        worker: tokio::spawn(worker.run()),
    })
}

pub(in crate::queue) async fn allocate(
    handle: &AsyncDeviceHandle,
    length: usize,
) -> TestResult<MappedDmaRegion> {
    Ok(handle.allocate_memory_region(length).await?)
}

pub(in crate::queue) async fn run_descriptor(
    fixture: Fixture,
    bytes: [u8; 64],
    destinations: &[MappedDmaRegion],
) -> TestResult<RunResult> {
    fixture.handle.enable_tracing().await;
    fixture
        .handle
        .write_memory(fixture.submission, &bytes)
        .await?;
    let endpoint = fixture.handle.register_endpoint();
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::SQ_PRODUCER.offset())?,
        1,
    )
    .await?;
    write(
        &endpoint,
        queue_offset(0, QueueRegisters::DOORBELL.offset())?,
        1,
    )
    .await?;
    let mut snapshot = fixture.handle.snapshot().await;
    while snapshot.steps + snapshot.errors + snapshot.stalls == 0 {
        snapshot = fixture.handle.wait_for_change_after(snapshot).await;
    }
    let consumer = read(
        &endpoint,
        queue_offset(0, QueueRegisters::SQ_CONSUMER.offset())?,
    )
    .await?;
    let completion = read(
        &endpoint,
        queue_offset(0, QueueRegisters::CQ_PRODUCER.offset())?,
    )
    .await?;
    let event = read(
        &endpoint,
        queue_offset(0, QueueRegisters::EQ_PRODUCER.offset())?,
    )
    .await?;
    let record = match (completion, event) {
        (1, 0) => Some(Record::Completion(AbiCompletion::decode(
            &fixture.handle.read_memory(fixture.completion, 32).await?,
        )?)),
        (0, 1) => Some(Record::Event(AbiEvent::decode(
            &fixture.handle.read_memory(fixture.event, 64).await?,
        )?)),
        (0, 0) => None,
        _ => return Err("unexpected retirement counts".into()),
    };
    let mut output = Vec::new();
    for region in destinations {
        output.push(
            fixture
                .handle
                .read_memory(region.base(), region.length())
                .await?,
        );
    }
    let traces = fixture.handle.trace_events().await;
    let native_steps = fixture.handle.native_steps().await;
    fixture.handle.request_shutdown().await;
    fixture.worker.await?;
    Ok(RunResult {
        consumer: RingCursor::new(consumer),
        record,
        destinations: output,
        steps: snapshot.steps,
        traces,
        native_steps,
    })
}

pub(in crate::queue) fn encode(command: AbiCommand) -> TestResult<[u8; 64]> {
    let mut bytes = [0; 64];
    command.encode(&mut bytes)?;
    Ok(bytes)
}

pub(in crate::queue) fn published_event(result: &RunResult) -> TestResult<AbiEvent> {
    match result.record {
        Some(Record::Event(event)) => Ok(event),
        Some(Record::Completion(_)) | None => Err("expected published DMA event".into()),
    }
}
