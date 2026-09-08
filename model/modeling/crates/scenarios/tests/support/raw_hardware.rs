#![expect(
    clippy::expect_used,
    reason = "Raw test fixtures fail immediately when setup is invalid."
)]

use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::{
    AbiCommand, AbiCommandPayloadCopyValues, AbiCommandPayloadValues, AbiCompletion, AbiEvent,
    DmaAddress, U32WordValues,
    registers::{QueueRegisters, queue_offset},
};
use pdma_model_device::{AsyncDeviceHandle, MmioRegisterEndpoint};

#[derive(Debug)]
pub(crate) struct CopyMemory {
    pub(crate) completion: DmaAddress,
    pub(crate) destination: DmaAddress,
}

pub(crate) async fn queue_copy(handle: &AsyncDeviceHandle, source: &[u8]) -> CopyMemory {
    let sq = handle
        .allocate_memory_region(4 * AbiCommand::RECORD_SIZE)
        .await
        .expect("submission ring allocation");
    let cq = handle
        .allocate_memory_region(4 * AbiCompletion::RECORD_SIZE)
        .await
        .expect("completion ring allocation");
    let eq = handle
        .allocate_memory_region(4 * AbiEvent::RECORD_SIZE)
        .await
        .expect("event ring allocation");
    let src = handle
        .allocate_memory_region(source.len())
        .await
        .expect("source allocation");
    let dst = handle
        .allocate_memory_region(source.len())
        .await
        .expect("destination allocation");
    let endpoint = handle.register_endpoint();
    for (offset, address) in [
        (QueueRegisters::SQ_BASE.offset(), sq.base()),
        (QueueRegisters::CQ_BASE.offset(), cq.base()),
        (QueueRegisters::EQ_BASE.offset(), eq.base()),
    ] {
        write_queue(&endpoint, offset, &address.raw().to_le_bytes()).await;
    }
    for offset in [
        QueueRegisters::SQ_DEPTH.offset(),
        QueueRegisters::CQ_DEPTH.offset(),
        QueueRegisters::EQ_DEPTH.offset(),
    ] {
        write_queue(&endpoint, offset, &4_u32.to_le_bytes()).await;
    }
    write_queue(
        &endpoint,
        QueueRegisters::ENABLE.offset(),
        &1_u32.to_le_bytes(),
    )
    .await;
    handle
        .write_memory(src.base(), source)
        .await
        .expect("source write");
    let command = AbiCommand::from_payload(
        0,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: src.base().raw(),
            destination_dma_address: dst.base().raw(),
            length: U32WordValues::new(u32::try_from(source.len()).expect("copy length fits ABI")),
        }),
    );
    let mut bytes = [0; AbiCommand::RECORD_SIZE];
    command
        .encode(&mut bytes)
        .expect("copy descriptor encoding");
    handle
        .write_memory(sq.base(), &bytes)
        .await
        .expect("SQ write");
    for offset in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::DOORBELL.offset(),
    ] {
        write_queue(&endpoint, offset, &1_u64.to_le_bytes()).await;
    }
    CopyMemory {
        completion: cq.base(),
        destination: dst.base(),
    }
}

pub(crate) async fn completion(
    handle: &AsyncDeviceHandle,
    address: DmaAddress,
) -> Option<AbiCompletion> {
    let mut producer = [0; 8];
    handle
        .register_endpoint()
        .read(
            queue_offset(0, QueueRegisters::CQ_PRODUCER.offset()).expect("CQ producer"),
            &mut producer,
        )
        .await
        .expect("CQ producer read");
    if u64::from_le_bytes(producer) == 0 {
        return None;
    }
    let bytes = handle
        .read_memory(
            address,
            u64::try_from(AbiCompletion::RECORD_SIZE).expect("record size"),
        )
        .await
        .expect("completion memory read");
    Some(AbiCompletion::decode(&bytes).expect("valid completion record"))
}

async fn write_queue(endpoint: &MmioRegisterEndpoint, offset: usize, bytes: &[u8]) {
    endpoint
        .write(queue_offset(0, offset).expect("queue register"), bytes)
        .await
        .expect("queue register write");
}
