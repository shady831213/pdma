#![allow(missing_docs, reason = "integration-test oracle")]

#[rustfmt::skip]
mod pin {

use hardware_interface_runtime::{AccessOrigin, RegisterAccess, RegisterObservation, RegisterObserver};
use pdma_model_contract::abi::mmio::{GlobalResponseRegistersView, IrqResponseRegistersView, MMIO_REGION_SIZE, MmioAccess, MmioAccessError, MmioReset, MmioResult, PdmaMmioMap, QueueResponseRegistersView};
use pdma_model_contract::semantic::{Command, CommandId, CopyRequest, DeviceEpoch, EventRouting, QueueActivation, QueueConfig, QueueControl, QueueId, QueueRingObjects, ResourceConfig, RingCursor, SoftwareOwnerId, WholeDeviceReset};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle, DeviceCore, DmaWrite, MmioRegisterError, PdmaBatchError, QueueBootstrapBatch, SubmissionBatch};
use std::sync::{Arc, Mutex};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const OFFSETS: [usize; 66] = [
    0x000,0x004,0x008,0x010,0x018,0x020,0x024,0x028,0x030,0x038,0x040,0x044,0x048,0x050,0x058,0x05c,0x060,0x064,0x068,
    0x100,0x108,0x110,0x118,0x120,0x124,0x128,0x130,0x138,0x140,0x148,0x150,0x158,0x160,0x168,0x16c,0x170,0x178,0x180,0x188,0x190,0x198,0x1a0,0x1a8,0x1b0,0x1b8,0x1c0,0x1c8,
    0x200,0x208,0x210,0x214,0x218,0x220,0x224,0x228,0x230,0x238,0x240,0x248,0x250,0x254,0x258,0x25c,0x260,0x268,0x270,
];
const WIDTHS: [usize; 66] = [
    4,4,4,8,8,4,4,8,8,8,4,4,8,8,4,4,4,4,4,
    8,8,8,8,4,4,4,8,8,4,8,4,8,4,4,4,8,8,8,8,8,8,8,8,8,8,8,8,
    8,8,4,4,8,4,4,8,8,8,8,8,4,4,4,4,8,8,8,
];
const NAMES: &str = "G_MAGIC G_MMIO_ABI_VERSION G_REQ_ABI_VERSION G_REQ_FEATURES G_RESET_EXPECTED_EPOCH G_COMMAND G_RESULT G_DETAIL0 G_DETAIL1 G_DETAIL2 G_RSP_ABI_VERSION G_RSP_RUN_STATE G_RSP_FEATURES G_RSP_EPOCH G_MAX_RING_DEPTH G_MAX_INDIRECT_DESTINATIONS G_MAX_TRANSFER_BYTES G_MAX_STRIDED_ROWS G_MAX_STRIDE Q_EXPECTED_EPOCH Q_ID Q_EXPECTED_CURSOR Q_NEW_CURSOR Q_CFG_SQ_DEPTH Q_CFG_CQ_DEPTH Q_CFG_EVENT_POLICY Q_CFG_SOFTWARE_OWNER Q_SQ_BASE Q_SQ_DEPTH Q_CQ_BASE Q_CQ_DEPTH Q_EQ_BASE Q_EQ_DEPTH Q_COMMAND Q_RESULT Q_DETAIL0 Q_DETAIL1 Q_DETAIL2 Q_RSP_EPOCH Q_RSP_ID Q_RSP_SQ_PRODUCER Q_RSP_SQ_CAPTURED Q_RSP_SQ_CONSUMER Q_RSP_CQ_PRODUCER Q_RSP_CQ_CONSUMER Q_RSP_EQ_PRODUCER Q_RSP_EQ_CONSUMER I_EXPECTED_EPOCH I_SOURCE_ID I_SOURCE_KIND I_ROUTE_KIND I_ROUTE_VECTOR I_COMMAND I_RESULT I_DETAIL0 I_DETAIL1 I_DETAIL2 I_RSP_EPOCH I_RSP_SOURCE_ID I_RSP_SOURCE_KIND I_RSP_STATE I_RSP_ROUTE_KIND I_RSP_NOTIFICATION_KIND I_RSP_VECTOR I_RSP_MESSAGE_ID I_RSP_PENDING_MESSAGES";

#[derive(Debug, Default)] struct Recorder(Mutex<Vec<RegisterObservation>>);
impl Recorder { fn snapshot(&self) -> Vec<RegisterObservation> { self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone() } }
impl RegisterObserver for Recorder { fn observe(&self, item: RegisterObservation) { self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(item); } }
fn fresh() -> (AsyncDevice, pdma_model_device::AsyncDeviceWorker) { AsyncDevice::from_core(DeviceCore::new(Arc::new(hardware_interface_runtime::SharedMemory::new()), ResourceConfig::default())) }
async fn read_number(handle: &AsyncDeviceHandle, offset: u32, width: usize) -> TestResult<u64> {
    let bytes = handle.read_mmio(offset, width as u128).await?;
    Ok(match width { 4 => u64::from(u32::from_le_bytes(bytes.try_into().map_err(|_| "u32")?)), 8 => u64::from_le_bytes(bytes.try_into().map_err(|_| "u64")?), _ => return Err("width".into()) })
}
async fn write_number(handle: &AsyncDeviceHandle, offset: u32, width: usize, value: u64) -> TestResult { match width { 4 => handle.write_mmio(offset, &u32::try_from(value)?.to_le_bytes()).await?, 8 => handle.write_mmio(offset, &value.to_le_bytes()).await?, _ => return Err("width".into()) } Ok(()) }
fn access(offset: usize) -> MmioAccess { if matches!(offset, 0x020 | 0x168 | 0x220) { MmioAccess::WriteOnly } else if matches!(offset, 0x000 | 0x004) || (0x024..=0x068).contains(&offset) || (0x16c..=0x1c8).contains(&offset) || (0x224..=0x270).contains(&offset) { MmioAccess::ReadOnly } else { MmioAccess::ReadWrite } }
fn mask(offset: usize, width: usize) -> u64 { match access(offset) { MmioAccess::ReadOnly => 0, MmioAccess::WriteOnly => u64::from(u32::MAX), MmioAccess::ReadWrite => match offset { 0x008 => 0xffff, 0x128 => 0, 0x210 | 0x214 => 1, _ if width == 4 => u64::from(u32::MAX), _ => u64::MAX } } }
fn activation(stage: &pdma_model_device::PdmaMemoryStage) -> TestResult<QueueActivation> { let [sq, cq, eq] = stage.regions() else { return Err("three rings".into()) }; Ok(QueueActivation::new(DeviceEpoch::initial(), QueueId::new(0), QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)), QueueRingObjects::new(sq.object(), cq.object(), eq.object()))) }
async fn response(handle: &AsyncDeviceHandle, entries: &[(u32, usize)]) -> TestResult<[u64; 9]> { let mut values = [0; 9]; for (slot, &(offset, width)) in values.iter_mut().zip(entries) { *slot = read_number(handle, offset, width).await?; } Ok(values) }
async fn readable_image(handle: &AsyncDeviceHandle) -> TestResult<Vec<Vec<u8>>> { let mut image = Vec::new(); for (&offset, &width) in OFFSETS.iter().zip(&WIDTHS) { if access(offset) != MmioAccess::WriteOnly { image.push(handle.read_mmio(u32::try_from(offset)?, width as u128).await?); } } Ok(image) }
async fn response_bytes(handle: &AsyncDeviceHandle, entries: &[(u32, usize)]) -> TestResult<Vec<u8>> { let mut bytes = Vec::new(); for &(offset, width) in entries { bytes.extend(handle.read_mmio(offset, width as u128).await?); } Ok(bytes) }

#[tokio::test(flavor = "current_thread")]
async fn authoritative_image_all_66_metadata_rows_reset_reads_and_request_writes() -> TestResult {
    // Given: literal v0.9 rows and a fresh public device endpoint.
    let resolved = PdmaMmioMap::resolve()?; let names: Vec<_> = NAMES.split_whitespace().collect(); let core = DeviceCore::new(Arc::new(hardware_interface_runtime::SharedMemory::new()), ResourceConfig::default()); let (device, worker) = AsyncDevice::from_core(core); let worker = tokio::spawn(worker.run()); let handle = device.handle();
    // When: every row is resolved and every readable/reset or writable/request register is used.
    assert_eq!((resolved.registers().len(), MMIO_REGION_SIZE, PdmaMmioMap::EXTENT), (66, 0x1000, 0x1000));
    for (((register, &offset), &width), name) in resolved.registers().iter().zip(&OFFSETS).zip(&WIDTHS).zip(&names) {
        let definition = register.definition(); let reset = if offset < 8 { MmioReset::Constant } else if offset == 0x050 { MmioReset::PostReset } else { MmioReset::Zero }; let reset_value = match offset { 0x000 => 0x414d_4450, 0x004 => 1, _ => 0 };
        assert_eq!((register.name(), register.offset(), definition.width(), definition.access(), definition.reset_kind(), definition.reset_value(), definition.writable_mask()), (*name, offset, width, access(offset), reset, reset_value, mask(offset, width)));
        if access(offset) != MmioAccess::WriteOnly { assert_eq!(read_number(&handle, u32::try_from(offset)?, width).await?, reset_value); }
        if access(offset) == MmioAccess::ReadWrite { let value = mask(offset, width) & 1; write_number(&handle, u32::try_from(offset)?, width, value).await?; assert_eq!(read_number(&handle, u32::try_from(offset)?, width).await?, value); }
    }
    write_number(&handle, 0x008, 4, 1).await?; write_number(&handle, 0x010, 8, 0).await?; write_number(&handle, 0x020, 4, 1).await?; let global_bytes = response_bytes(&handle, &[(0x040,4),(0x044,4),(0x048,8),(0x050,8),(0x058,4),(0x05c,4),(0x060,4),(0x064,4),(0x068,4)]).await?; let global = GlobalResponseRegistersView::new(&global_bytes)?; assert_eq!((global.abi_version()?.value()?, global.run_state()?.value()?, global.features()?.value()?, global.epoch()?.value()?), (1,0,0xff,0));
    let stage = handle.stage_memory_regions(&[128,64,128]).await?; let activation = activation(&stage)?; handle.bootstrap_queue_batch(QueueBootstrapBatch::new(stage,activation)).await?; let queue_bytes = response_bytes(&handle, &[(0x188,8),(0x190,8),(0x198,8),(0x1a0,8),(0x1a8,8),(0x1b0,8),(0x1b8,8),(0x1c0,8),(0x1c8,8)]).await?; let queue = QueueResponseRegistersView::new(&queue_bytes)?; assert_eq!((queue.epoch()?.value()?,queue.queue_id()?.value()?,queue.sq_producer()?.value()?,queue.sq_captured()?.value()?,queue.sq_consumer()?.value()?,queue.cq_producer()?.value()?,queue.cq_consumer()?.value()?,queue.eq_producer()?.value()?,queue.eq_consumer()?.value()?), (0,0,0,0,0,0,0,0,0));
    write_number(&handle,0x200,8,0).await?; write_number(&handle,0x208,8,0).await?; write_number(&handle,0x210,4,0).await?; write_number(&handle,0x220,4,7).await?; let irq_bytes = response_bytes(&handle,&[(0x240,8),(0x248,8),(0x250,4),(0x254,4),(0x258,4),(0x25c,4),(0x260,8),(0x268,8),(0x270,8)]).await?; let irq = IrqResponseRegistersView::new(&irq_bytes)?; assert_eq!((irq.epoch()?.value()?,irq.source_id()?.value()?,irq.source_kind()?.value()?,irq.state()?.value()?,irq.route_kind()?.value()?,irq.notification_kind()?.value()?,irq.vector()?.value()?,irq.message_id()?.value()?,irq.pending_messages()?.value()?), (0,0,0,5,0,0,0,0,0));
    device.request_shutdown().await; worker.await?;
    // Then: the endpoint exposes every metadata row, raw request, and compact response from a spawned actor.
    println!("authoritative_endpoint extent=4096 metadata_rows=66 compact=global,queue,irq actor=spawned_joined"); Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn global_happy_commands_batches_generations_and_clone_order_match_semantics() -> TestResult {
    // Given: one observed device, one direct clone access, and one staged bootstrap.
    let observer = Arc::new(Recorder::default()); let (device, _worker) = AsyncDevice::from_core_observed(DeviceCore::new(Arc::new(hardware_interface_runtime::SharedMemory::new()), ResourceConfig::default()), observer.clone()); let handle = device.handle();
    let mut magic = [0; 4]; handle.register_endpoint().clone().read(0, &mut magic).await?; let stage = handle.stage_memory_regions(&[128, 64, 128]).await?; let sq = *stage.regions().first().ok_or("submission ring")?; let activation = activation(&stage)?; let queue = activation.queue(); let before = handle.snapshot().await;
    // When: both closed batches, all global commands, and typed queue status execute.
    let bootstrap = handle.bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation)).await?; assert_eq!(handle.wait_for_change_after(before).await.generation(), before.generation() + 1);
    let before_submission = handle.snapshot().await; handle.submit_queue_batch(SubmissionBatch::new(queue, RingCursor::new(0), DmaWrite::new(sq.base(), vec![0x6b; 64]), None)).await?; assert_eq!(handle.wait_for_change_after(before_submission).await.generation(), before_submission.generation() + 1);
    let committed = (handle.snapshot().await, handle.memory_generation().await, handle.read_memory(sq.base(), 64).await?, observer.snapshot().len()); let malformed = SubmissionBatch::new(queue, RingCursor::new(1), DmaWrite::new(pdma_model_contract::abi::DmaAddress::new(sq.base().raw() + 64), vec![0; 63]), None); assert_eq!(handle.submit_queue_batch(malformed).await, Err(PdmaBatchError::InvalidShape)); assert_eq!((handle.snapshot().await, handle.memory_generation().await, handle.read_memory(sq.base(), 64).await?, observer.snapshot().len()), committed);
    write_number(&handle, 0x008, 4, 1).await?; write_number(&handle, 0x020, 4, 1).await?; let discovered = [read_number(&handle, 0x040, 4).await?, read_number(&handle, 0x048, 8).await?]; write_number(&handle, 0x020, 4, 0xfeed).await?; assert_eq!([read_number(&handle, 0x040, 4).await?, read_number(&handle, 0x048, 8).await?], discovered); write_number(&handle, 0x020, 4, 2).await?; write_number(&handle, 0x020, 4, 3).await?; let semantic = handle.control_queue(QueueControl::ReadStatus(queue)).await?;
    // Then: replacement/preservation, exact batch responses, parity, and global order are literal.
    assert_eq!(magic, 0x414d_4450_u32.to_le_bytes()); assert_eq!(bootstrap.as_bytes().len(), 72); assert_eq!(response(&handle, &[(0x188,8),(0x190,8),(0x198,8),(0x1a0,8),(0x1a8,8),(0x1b0,8),(0x1b8,8),(0x1c0,8),(0x1c8,8)]).await?, [semantic.queue().epoch().raw(),0,1,0,0,0,0,0,0]); assert_eq!(handle.current_status().await.epoch(), DeviceEpoch::initial());
    let observations = observer.snapshot(); let [first, second, third, ..] = observations.as_slice() else { return Err("three observations".into()) }; assert_eq!((first.sequence, first.origin, second.sequence, second.origin, third.sequence, third.origin), (0,AccessOrigin::Direct,1,AccessOrigin::Batch,2,AccessOrigin::Batch)); println!("happy global_queue_responses=replace,preserve batches=atomic generation_delta=2 observations=ordered"); Ok(())
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
    assert_eq!((epoch,handle.read_buffer(to,0,1).await?,handle.memory_generation().await,handle.trace_events().await,handle.snapshot().await.steps),(DeviceEpoch::new(1),vec![0x6d],memory_generation,trace,before.steps)); assert_eq!((read_number(&handle,0x108,8).await?,read_number(&handle,0x208,8).await?,read_number(&handle,0x16c,4).await?,read_number(&handle,0x224,4).await?),(0,0,0,0)); assert!(matches!(handle.ack_irq_source(source).await,Err(pdma_model_device::ModelError::StaleDeviceEpoch{..}))); device.request_shutdown().await; worker.await?; println!("happy irq_transcript=[0,0,0,75,1,2,41,0,0],EMPTY reset_epoch=1 persistent=dma,trace,steps"); Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn response_adverse_errors_batches_and_dropped_futures_are_non_mutating() -> TestResult {
    // Given: a clone-observed device, a stale stage, and complete public state snapshots.
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new()); let observer = Arc::new(Recorder::default()); let (device,_worker) = AsyncDevice::from_core_observed(DeviceCore::new(memory.clone(),ResourceConfig::default()),observer.clone()); let handle = device.handle(); let stale = handle.stage_memory_regions(&[128,64,128]).await?; let stale_activation = activation(&stale)?; handle.allocate_memory_region(1).await?; let before = handle.snapshot().await; let generation = handle.memory_generation().await; let regions = memory.regions(); let registers = readable_image(&handle).await?; let observations = observer.snapshot().len();
    // When: malformed accesses, a stale batch, and an unpolled valid batch are rejected/dropped.
    let endpoint = handle.register_endpoint(); let mut four = [0;4]; let mut eight = [0;8]; assert_eq!(endpoint.read(0x006,&mut four).await,Err(MmioRegisterError::Access(MmioAccessError::Misaligned{offset:0x006,width:4}))); assert_eq!(endpoint.read(0,&mut eight).await,Err(MmioRegisterError::Access(MmioAccessError::WrongWidth{offset:0,expected:4,actual:8}))); assert_eq!(endpoint.write(0,&[0;4]).await,Err(MmioRegisterError::Access(MmioAccessError::WriteToReadOnly{offset:0}))); assert_eq!(endpoint.write(0x128,&1_u32.to_le_bytes()).await,Err(MmioRegisterError::Access(MmioAccessError::ReservedBits{offset:0x128,bits:1})));
    assert!(matches!(handle.bootstrap_queue_batch(QueueBootstrapBatch::new(stale,stale_activation)).await,Err(PdmaBatchError::Dma(details)) if details.result == MmioResult::DmaStaleStage && details.details == [0,1,0])); let cancelled = handle.stage_memory_regions(&[128,64,128]).await?; let cancelled_activation = activation(&cancelled)?; drop(handle.bootstrap_queue_batch(QueueBootstrapBatch::new(cancelled,cancelled_activation)));
    // Then: error precedence is exact and every public state partition is unchanged.
    assert_eq!(observer.snapshot().len(),observations); assert_eq!((handle.snapshot().await,handle.memory_generation().await,memory.regions(),readable_image(&handle).await?,handle.trace_events().await,handle.irq_notification_ready().await),(before,generation,regions,registers,Vec::new(),false));
    let active = handle.stage_memory_regions(&[128,64,128]).await?; let sq = *active.regions().first().ok_or("submission ring")?; let active_activation = activation(&active)?; let queue = active_activation.queue(); handle.bootstrap_queue_batch(QueueBootstrapBatch::new(active,active_activation)).await?; let command_before = (handle.snapshot().await,handle.memory_generation().await,handle.read_memory(sq.base(),64).await?,response(&handle,&[(0x188,8),(0x190,8),(0x198,8),(0x1a0,8),(0x1a8,8),(0x1b0,8),(0x1b8,8),(0x1c0,8),(0x1c8,8)]).await?); let rejected = handle.submit_queue_batch(SubmissionBatch::new(queue,RingCursor::new(1),DmaWrite::new(sq.base(),vec![0xaa;64]),None)).await; assert!(matches!(rejected,Err(PdmaBatchError::Command(details)) if details.result == MmioResult::InvalidTransaction)); assert_eq!((handle.snapshot().await,handle.memory_generation().await,handle.read_memory(sq.base(),64).await?,response(&handle,&[(0x188,8),(0x190,8),(0x198,8),(0x1a0,8),(0x1a8,8),(0x1b0,8),(0x1b8,8),(0x1c0,8),(0x1c8,8)]).await?),command_before);
    println!("adverse errors=stale,misaligned,wrong-width,access,mask,command drop=precommit unchanged=memory,mappings,registers,queues,irq,trace,generations,observations"); Ok(())
}
}
