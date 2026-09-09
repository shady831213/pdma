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
use pdma_model_host::adapter::abi::{PdmaHostMemory, PdmaRegionRequest};

#[derive(Debug)]
pub(crate) struct CopyMemory {
    pub(crate) completion: DmaAddress,
    pub(crate) destination: DmaAddress,
}

pub(crate) async fn queue_copy(
    handle: &AsyncDeviceHandle,
    memory: &PdmaHostMemory,
    source: &[u8],
) -> CopyMemory {
    let alignment = hardware_interface_runtime::MemoryAlignment::try_new(64).expect("alignment");
    let allocate = |length: usize| {
        let region = memory
            .allocate_region(PdmaRegionRequest::new(
                u64::try_from(length).expect("length"),
                alignment,
            ))
            .expect("fixture allocation");
        DmaAddress::new(region.base().raw())
    };
    let sq = allocate(4 * AbiCommand::RECORD_SIZE);
    let cq = allocate(4 * AbiCompletion::RECORD_SIZE);
    let eq = allocate(4 * AbiEvent::RECORD_SIZE);
    let src = allocate(source.len());
    let dst = allocate(source.len());
    let endpoint = handle.register_endpoint();
    for (offset, address) in [
        (QueueRegisters::SQ_BASE.offset(), sq),
        (QueueRegisters::CQ_BASE.offset(), cq),
        (QueueRegisters::EQ_BASE.offset(), eq),
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
        .write_memory(src, source)
        .await
        .expect("source write");
    let command = AbiCommand::from_payload(
        0,
        AbiCommandPayloadValues::Copy(AbiCommandPayloadCopyValues {
            source_dma_address: src.raw(),
            destination_dma_address: dst.raw(),
            length: U32WordValues::new(u32::try_from(source.len()).expect("copy length fits ABI")),
        }),
    );
    let mut bytes = [0; AbiCommand::RECORD_SIZE];
    command
        .encode(&mut bytes)
        .expect("copy descriptor encoding");
    handle.write_memory(sq, &bytes).await.expect("SQ write");
    for offset in [
        QueueRegisters::SQ_PRODUCER.offset(),
        QueueRegisters::DOORBELL.offset(),
    ] {
        write_queue(&endpoint, offset, &1_u64.to_le_bytes()).await;
    }
    CopyMemory {
        completion: cq,
        destination: dst,
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
