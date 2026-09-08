#![allow(missing_docs, reason = "integration-test oracle")]

#[rustfmt::skip]
mod pin {

use hardware_interface_runtime::{AccessOrigin, RegisterAccess, RegisterObservation, RegisterObserver};
use pdma_model_contract::abi::mmio::{GlobalResponseRegistersView, IrqResponseRegistersView, MMIO_REGION_SIZE, MmioAccess, MmioAccessError, MmioReset, MmioResult, PdmaMmioMap, QueueResponseRegistersView};
use pdma_model_contract::semantic::{Command, CommandId, CopyRequest, DeviceEpoch, EventRouting, QueueActivation, QueueConfig, QueueControl, QueueId, QueueRingObjects, ResourceConfig, RingCursor, SoftwareOwnerId, WholeDeviceReset};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, MmioRegisterError, PdmaMemoryError};
use std::sync::{Arc, Mutex};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const OFFSETS: [usize; 66] = [
    0x000,0x004,0x008,0x010,0x018,0x020,0x024,0x028,0x030,0x038,0x040,0x044,0x048,0x050,0x058,0x05c,0x060,0x064,0x068,
    0x100,0x108,0x110,0x118,0x120,0x124,0x128,0x130,0x138,0x140,0x148,0x150,0x158,0x160,0x164,0x168,0x170,0x178,0x180,0x188,0x190,0x198,0x1a0,0x1a8,0x1b0,0x1b8,0x1c0,0x1c8,
    0x200,0x208,0x210,0x214,0x218,0x220,0x224,0x228,0x230,0x238,0x240,0x248,0x250,0x254,0x258,0x25c,0x260,0x268,0x270,
];
const WIDTHS: [usize; 66] = [
    4,4,4,8,8,4,4,8,8,8,4,4,8,8,4,4,4,4,4,
    8,8,8,8,4,4,4,8,8,4,8,4,8,4,4,4,8,8,8,8,8,8,8,8,8,8,8,8,
    8,8,4,4,8,4,4,8,8,8,8,8,4,4,4,4,8,8,8,
];
const NAMES: &str = "MAGIC MMIO_ABI_VERSION REQ_ABI_VERSION REQ_FEATURES RESET_EXPECTED_EPOCH COMMAND RESULT DETAIL0 DETAIL1 DETAIL2 ABI_VERSION RUN_STATE FEATURES EPOCH MAX_RING_DEPTH MAX_INDIRECT_DESTINATIONS MAX_TRANSFER_BYTES MAX_STRIDED_ROWS MAX_STRIDE EXPECTED_EPOCH ID EXPECTED_CURSOR NEW_CURSOR CFG_SQ_DEPTH CFG_CQ_DEPTH CFG_EVENT_POLICY CFG_SOFTWARE_OWNER SQ_BASE SQ_DEPTH CQ_BASE CQ_DEPTH EQ_BASE EQ_DEPTH COMMAND RESULT DETAIL0 DETAIL1 DETAIL2 EPOCH QUEUE_ID SQ_PRODUCER SQ_CAPTURED SQ_CONSUMER CQ_PRODUCER CQ_CONSUMER EQ_PRODUCER EQ_CONSUMER EXPECTED_EPOCH SOURCE_ID SOURCE_KIND ROUTE_KIND ROUTE_VECTOR COMMAND RESULT DETAIL0 DETAIL1 DETAIL2 EPOCH SOURCE_ID SOURCE_KIND STATE ROUTE_KIND NOTIFICATION_KIND VECTOR MESSAGE_ID PENDING_MESSAGES";

#[derive(Debug, Default)] struct Recorder(Mutex<Vec<RegisterObservation>>);
impl Recorder { fn snapshot(&self) -> Vec<RegisterObservation> { self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone() } }
impl RegisterObserver for Recorder { fn observe(&self, item: RegisterObservation) { self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(item); } }
fn fresh() -> (AsyncDevice, pdma_model_device::AsyncDeviceWorker) { AsyncDevice::new(pdma_model_device::PdmaMemoryProvisioning::shared(Arc::new(hardware_interface_runtime::SharedMemory::new())), ResourceConfig::default()) }
async fn read_number(handle: &AsyncDeviceHandle, offset: u32, width: usize) -> TestResult<u64> {
    let bytes = handle.read_mmio(offset, width as u128).await?;
    Ok(match width { 4 => u64::from(u32::from_le_bytes(bytes.try_into().map_err(|_| "u32")?)), 8 => u64::from_le_bytes(bytes.try_into().map_err(|_| "u64")?), _ => return Err("width".into()) })
}
async fn write_number(handle: &AsyncDeviceHandle, offset: u32, width: usize, value: u64) -> TestResult { match width { 4 => handle.register_endpoint().write(offset, &u32::try_from(value)?.to_le_bytes()).await?, 8 => handle.register_endpoint().write(offset, &value.to_le_bytes()).await?, _ => return Err("width".into()) } Ok(()) }
fn access(offset: usize) -> MmioAccess { if matches!(offset, 0x020 | 0x164 | 0x220) { MmioAccess::WriteOnly } else if matches!(offset, 0x000 | 0x004) || (0x024..=0x068).contains(&offset) || (0x168..=0x1c8).contains(&offset) || (0x224..=0x270).contains(&offset) { MmioAccess::ReadOnly } else { MmioAccess::ReadWrite } }
fn mask(offset: usize, width: usize) -> u64 { match access(offset) { MmioAccess::ReadOnly => 0, MmioAccess::WriteOnly => u64::from(u32::MAX), MmioAccess::ReadWrite => match offset { 0x008 => 0xffff, 0x128 => 0, 0x210 | 0x214 => 1, _ if width == 4 => u64::from(u32::MAX), _ => u64::MAX } } }
fn activation(stage: &pdma_model_device::PdmaMemoryStage) -> TestResult<QueueActivation> { let [sq, cq, eq] = stage.regions() else { return Err("three rings".into()) }; Ok(QueueActivation::new(DeviceEpoch::initial(), QueueId::new(0), QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)), QueueRingObjects::new(sq.object(), cq.object(), eq.object()))) }
async fn response(handle: &AsyncDeviceHandle, entries: &[(u32, usize)]) -> TestResult<[u64; 9]> { let mut values = [0; 9]; for (slot, &(offset, width)) in values.iter_mut().zip(entries) { *slot = read_number(handle, offset, width).await?; } Ok(values) }
async fn readable_image(handle: &AsyncDeviceHandle) -> TestResult<Vec<Vec<u8>>> { let mut image = Vec::new(); for (&offset, &width) in OFFSETS.iter().zip(&WIDTHS) { if access(offset) != MmioAccess::WriteOnly { image.push(handle.read_mmio(u32::try_from(offset)?, width as u128).await?); } } Ok(image) }
async fn response_bytes(handle: &AsyncDeviceHandle, entries: &[(u32, usize)]) -> TestResult<Vec<u8>> { let mut bytes = Vec::new(); for &(offset, width) in entries { bytes.extend(handle.read_mmio(offset, width as u128).await?); } Ok(bytes) }

#[tokio::test(flavor = "current_thread")]
async fn authoritative_image_all_66_metadata_rows_reset_reads_and_request_writes() -> TestResult {
    // Given: literal v0.9 rows and a fresh public device endpoint.
    let resolved = PdmaMmioMap::resolve()?; let names: Vec<_> = NAMES.split_whitespace().collect(); let (device, worker) = fresh(); let worker = tokio::spawn(worker.run()); let handle = device.handle();
    // When: every row is resolved and every readable/reset or writable/request register is used.
    assert_eq!((resolved.registers().len(), MMIO_REGION_SIZE, PdmaMmioMap::EXTENT), (66, 0x1000, 0x1000));
    for (((register, &offset), &width), name) in resolved.registers().iter().zip(&OFFSETS).zip(&WIDTHS).zip(&names) {
        let definition = register.definition(); let reset = if offset < 8 { MmioReset::Constant } else if offset == 0x050 { MmioReset::PostReset } else { MmioReset::Zero }; let reset_value = match offset { 0x000 => 0x414d_4450, 0x004 => 1, _ => 0 };
        assert_eq!((register.name(), register.offset(), definition.width(), definition.access(), definition.reset_kind(), definition.reset_value(), definition.writable_mask()), (*name, offset, width, access(offset), reset, reset_value, mask(offset, width)));
        if access(offset) != MmioAccess::WriteOnly { assert_eq!(read_number(&handle, u32::try_from(offset)?, width).await?, reset_value); }
        if access(offset) == MmioAccess::ReadWrite { let value = mask(offset, width) & 1; write_number(&handle, u32::try_from(offset)?, width, value).await?; assert_eq!(read_number(&handle, u32::try_from(offset)?, width).await?, value); }
    }
    write_number(&handle, 0x008, 4, 1).await?; write_number(&handle, 0x010, 8, 0).await?; write_number(&handle, 0x020, 4, 1).await?; let global_bytes = response_bytes(&handle, &[(0x040,4),(0x044,4),(0x048,8),(0x050,8),(0x058,4),(0x05c,4),(0x060,4),(0x064,4),(0x068,4)]).await?; let global = GlobalResponseRegistersView::new(&global_bytes)?; assert_eq!((global.abi_version()?.value()?, global.run_state()?.value()?, global.features()?.value()?, global.epoch()?.value()?), (1,0,0xff,0));
    let stage = handle.stage_memory_regions(&[128,64,128]).await?; let activation = activation(&stage)?; handle.commit_memory_stage(stage).await?; handle.activate_mapped_queue(activation).await?; write_number(&handle,0x100,8,0).await?; write_number(&handle,0x108,8,0).await?; write_number(&handle,0x164,4,2).await?; let queue_bytes = response_bytes(&handle, &[(0x188,8),(0x190,8),(0x198,8),(0x1a0,8),(0x1a8,8),(0x1b0,8),(0x1b8,8),(0x1c0,8),(0x1c8,8)]).await?; let queue = QueueResponseRegistersView::new(&queue_bytes)?; assert_eq!((queue.epoch()?.value()?,queue.queue_id()?.value()?,queue.sq_producer()?.value()?,queue.sq_captured()?.value()?,queue.sq_consumer()?.value()?,queue.cq_producer()?.value()?,queue.cq_consumer()?.value()?,queue.eq_producer()?.value()?,queue.eq_consumer()?.value()?), (0,0,0,0,0,0,0,0,0));
    write_number(&handle,0x200,8,0).await?; write_number(&handle,0x208,8,0).await?; write_number(&handle,0x210,4,0).await?; write_number(&handle,0x220,4,7).await?; let irq_bytes = response_bytes(&handle,&[(0x240,8),(0x248,8),(0x250,4),(0x254,4),(0x258,4),(0x25c,4),(0x260,8),(0x268,8),(0x270,8)]).await?; let irq = IrqResponseRegistersView::new(&irq_bytes)?; assert_eq!((irq.epoch()?.value()?,irq.source_id()?.value()?,irq.source_kind()?.value()?,irq.state()?.value()?,irq.route_kind()?.value()?,irq.notification_kind()?.value()?,irq.vector()?.value()?,irq.message_id()?.value()?,irq.pending_messages()?.value()?), (0,0,0,5,0,0,0,0,0));
    device.request_shutdown().await; worker.await?;
    // Then: the endpoint exposes every metadata row, raw request, and compact response from a spawned actor.
    println!("authoritative_endpoint extent=4096 metadata_rows=66 compact=global,queue,irq actor=spawned_joined"); Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn global_commands_and_individual_queue_accesses_match_semantics() -> TestResult {
    // Given: observed direct endpoint clones and independently committed ring mappings.
    let observer = Arc::new(Recorder::default());
    let (device, _worker) = AsyncDevice::new_observed(pdma_model_device::PdmaMemoryProvisioning::shared(Arc::new(hardware_interface_runtime::SharedMemory::new())), ResourceConfig::default(), observer.clone());
    let handle = device.handle();
    let mut magic = [0; 4];
    handle.register_endpoint().clone().read(0, &mut magic).await?;
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let sq = *stage.regions().first().ok_or("submission ring")?;
    let activation = activation(&stage)?;
    let queue = activation.queue();
    handle.commit_memory_stage(stage).await?;
    handle.activate_mapped_queue(activation).await?;
    handle.write_memory(sq.base(), &[0x6b; 64]).await?;

    // When: raw producer publication and all global commands execute independently.
    for (offset, value) in [(0x100,0_u64),(0x108,0),(0x110,0),(0x118,1)] {
        write_number(&handle,offset,8,value).await?;
    }
    let before = handle.snapshot().await;
    write_number(&handle,0x164,4,3).await?;
    assert_eq!(handle.wait_for_change_after(before).await.generation(), before.generation() + 1);
    write_number(&handle, 0x008, 4, 1).await?;
    write_number(&handle, 0x020, 4, 1).await?;
    let discovered = [read_number(&handle, 0x040, 4).await?, read_number(&handle, 0x048, 8).await?];
    write_number(&handle, 0x020, 4, 0xfeed).await?;
    assert_eq!([read_number(&handle, 0x040, 4).await?, read_number(&handle, 0x048, 8).await?], discovered);
    write_number(&handle, 0x020, 4, 2).await?;
    write_number(&handle, 0x020, 4, 3).await?;
    let semantic = handle.control_queue(QueueControl::ReadStatus(queue)).await?;

    // Then: the queue response, persistent bytes, and per-access observer order are exact.
    assert_eq!(magic, 0x414d_4450_u32.to_le_bytes());
    assert_eq!(semantic.submission().producer(), RingCursor::new(1));
    assert_eq!(response(&handle, &[(0x188,8),(0x190,8),(0x198,8),(0x1a0,8),(0x1a8,8),(0x1b0,8),(0x1b8,8),(0x1c0,8),(0x1c8,8)]).await?, [0,0,1,0,0,0,0,0,0]);
    assert_eq!(handle.read_memory(sq.base(),64).await?,vec![0x6b;64]);
    assert_eq!(handle.current_status().await.epoch(),DeviceEpoch::initial());
    for (sequence,item) in observer.snapshot().iter().enumerate() {
        assert_eq!((item.sequence,item.origin),(u64::try_from(sequence)?,AccessOrigin::Direct));
    }
    println!("happy accesses=individual response=exact observations=direct,ordered");
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn happy_irq_claim_transcript_and_reset_partitions_are_exact() -> TestResult {
    // Given: a traced message-routed queue with one completed copy.
    let (device, worker) = fresh(); let worker = tokio::spawn(worker.run()); let handle = device.handle(); handle.enable_tracing().await; let queue = handle.create_queue(QueueConfig::new(1,1,EventRouting::QueueLocal,SoftwareOwnerId::new(0))).await?; let source = queue.completion_irq_source();
    for (offset,width,value) in [(0x200,8,0),(0x208,8,u64::try_from(queue.completion_ring_id().raw())?),(0x210,4,0),(0x214,4,1),(0x218,8,41)] { write_number(&handle,offset,width,value).await?; } for command in [1,2,7,4,5] { write_number(&handle,0x220,4,command).await?; }
    let semantic_state = handle.observe_irq_source(source).await?.bits(); write_number(&handle,0x220,4,7).await?; assert_eq!(read_number(&handle,0x254,4).await?,u64::from(semantic_state)); let from = handle.allocate_buffer(1).await?; let to = handle.allocate_buffer(1).await?; handle.write_buffer(from,0,&[0x6d]).await?; handle.submit_command(queue,Command::new(CommandId::new(0),pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(from,to,1)))).await?; handle.ring_doorbell(queue).await?; device.wait_for_progress_after(0).await;
    // When: raw claim/empty and all remaining IRQ commands precede a typed reset.
    let irq_entries = [(0x240,8),(0x248,8),(0x250,4),(0x254,4),(0x258,4),(0x25c,4),(0x260,8),(0x268,8),(0x270,8)]; write_number(&handle,0x220,4,9).await?; assert_eq!(response(&handle,&irq_entries).await?,[0,0,0,75,1,2,41,0,0]); write_number(&handle,0x220,4,9).await?; assert_eq!((read_number(&handle,0x224,4).await?,response(&handle,&irq_entries).await?),(u64::from(MmioResult::Empty.code()),[0;9])); for command in [6,8,3] { write_number(&handle,0x220,4,command).await?; }
    let memory_generation = handle.memory_generation().await; let trace = handle.trace_events().await; let before = handle.snapshot().await; let epoch = handle.reset(WholeDeviceReset::new(DeviceEpoch::initial())).await?;
    // Then: reset clears transient state and preserves DMA, trace, accounting, and mappings.
    assert_eq!((epoch,handle.read_buffer(to,0,1).await?,handle.memory_generation().await,handle.trace_events().await,handle.snapshot().await.steps),(DeviceEpoch::new(1),vec![0x6d],memory_generation,trace,before.steps)); assert_eq!((read_number(&handle,0x108,8).await?,read_number(&handle,0x208,8).await?,read_number(&handle,0x168,4).await?,read_number(&handle,0x224,4).await?),(0,0,0,0)); assert!(matches!(handle.ack_irq_source(source).await,Err(pdma_model_device::ModelError::StaleDeviceEpoch{..}))); device.request_shutdown().await; worker.await?; println!("happy irq_transcript=[0,0,0,75,1,2,41,0,0],EMPTY reset_epoch=1 persistent=dma,trace,steps"); Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn rejected_accesses_preserve_prior_memory_and_register_progress() -> TestResult {
    // Given: a clone-observed endpoint and a stale standalone memory proposal.
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(Recorder::default());
    let (device,_worker) = AsyncDevice::new_observed(pdma_model_device::PdmaMemoryProvisioning::shared(memory.clone()), ResourceConfig::default(), observer.clone());
    let handle = device.handle();
    let stale = handle.stage_memory_regions(&[128,64,128]).await?;
    handle.allocate_memory_region(1).await?;
    assert_eq!(handle.commit_memory_stage(stale).await,Err(PdmaMemoryError::StaleStage{expected_generation:0,actual_generation:1}));
    let before = handle.snapshot().await;
    let generation = handle.memory_generation().await;
    let regions = memory.regions();
    let registers = readable_image(&handle).await?;
    let observations = observer.snapshot().len();
    let endpoint = handle.register_endpoint();
    let mut four = [0;4];
    let mut eight = [0;8];

    // When: malformed individual register accesses fail validation.
    assert_eq!(endpoint.read(0x006,&mut four).await,Err(MmioRegisterError::Access(MmioAccessError::Misaligned{offset:0x006,width:4})));
    assert_eq!(endpoint.read(0,&mut eight).await,Err(MmioRegisterError::Access(MmioAccessError::WrongWidth{offset:0,expected:4,actual:8})));
    assert_eq!(endpoint.write(0,&[0;4]).await,Err(MmioRegisterError::Access(MmioAccessError::WriteToReadOnly{offset:0})));
    assert_eq!(endpoint.write(0x128,&1_u32.to_le_bytes()).await,Err(MmioRegisterError::Access(MmioAccessError::ReservedBits{offset:0x128,bits:1})));

    // Then: rejected accesses emit no observation and preserve every captured partition.
    assert_eq!(observer.snapshot().len(),observations);
    assert_eq!((handle.snapshot().await,handle.memory_generation().await,memory.regions(),readable_image(&handle).await?,handle.trace_events().await,handle.irq_notification_ready().await),(before,generation,regions,registers,Vec::new(),false));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn rejected_producer_command_keeps_completed_descriptor_write() -> TestResult {
    // Given: committed queue memory and a descriptor already written through checked memory.
    let (device,_worker) = fresh();
    let handle = device.handle();
    let stage = handle.stage_memory_regions(&[128,64,128]).await?;
    let sq = *stage.regions().first().ok_or("submission ring")?;
    let activation = activation(&stage)?;
    let queue = activation.queue();
    handle.commit_memory_stage(stage).await?;
    handle.activate_mapped_queue(activation).await?;
    handle.write_memory(sq.base(),&[0xaa;64]).await?;
    let status = handle.control_queue(QueueControl::ReadStatus(queue)).await?;

    // When: a later register command supplies the wrong expected producer.
    for (offset,value) in [(0x100,0_u64),(0x108,0),(0x110,1),(0x118,2)] {
        write_number(&handle,offset,8,value).await?;
    }
    write_number(&handle,0x164,4,3).await?;

    // Then: only publication rejects; earlier bytes and request-register values remain.
    assert_eq!(read_number(&handle,0x168,4).await?,u64::from(MmioResult::InvalidTransaction.code()));
    assert_eq!(handle.control_queue(QueueControl::ReadStatus(queue)).await?,status);
    assert_eq!(handle.read_memory(sq.base(),64).await?,vec![0xaa;64]);
    assert_eq!(read_number(&handle,0x110,8).await?,1);
    Ok(())
}
}
