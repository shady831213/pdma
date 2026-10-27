use hardware_interface_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    CopyCommandDestinationValues, CopyCommandSourceValues, LinearEndpoint,
};

use super::{
    client,
    control::{ring, rings},
    *,
};
use hardware_interface_io::memory::SharedMemoryAllocator;
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{
    control::{ControlEvent, ControlEventKind, ControlResponseStatus},
    dma::{ChannelId, Command, Event},
    hw_components::control::EventRingErrorCode,
    memory::FunctionId,
};
use pdma_model_host::{Channel, ChannelConfig, Error, Management, Rings};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
mod vf;

#[tokio::test]
async fn fatal_recovery_retains_vf_grant_and_requires_explicit_reopen() {
    recovery(false).await;
}

#[tokio::test]
async fn fatal_event_overflow_survives_dma_reset_and_rearms_without_replay() {
    recovery(true).await;
}

async fn recovery(overflow: bool) {
    // Given: one VF owns the DMA channel; only PF has management/event interrupts.
    let backing = Arc::new(SharedMemory::new());
    let allocator = SharedMemoryAllocator::new(Arc::clone(&backing), MemoryAddress::new(0x1000));
    let vf_allocator = SharedMemoryAllocator::new(backing, MemoryAddress::new(0x0010_0000));
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 1, 2).unwrap();
    let vf = hosts.pop().unwrap();
    let pf = hosts.pop().unwrap();
    let shared = Arc::clone(&system.shared);
    let (trigger, mut injected) = mpsc::channel::<()>(1);
    let (grant, granted) = oneshot::channel();
    let (ready, prepared) = oneshot::channel();
    let (checks, requests) = mpsc::channel(1);
    let (resume, resumed) = oneshot::channel();
    let (finished, copied) = oneshot::channel();
    let injection = async {
        while injected.recv().await.is_some() {
            shared.access(|hardware| hardware.dma.signal_fatal()).await;
        }
    };
    let vf_program = vf::run(
        vf,
        vf_allocator,
        vf::Messages {
            grant: granted,
            ready,
            checks: requests,
            resume: resumed,
            finished,
        },
    );
    let pf_program = client::run(pf.runtime, async |interrupt| {
        let storage = rings(&allocator).unwrap();
        let mut management = Management::setup(
            pf.bus.clone(),
            allocator.memory_access(),
            client::config(storage, &interrupt),
        )
        .await
        .unwrap();
        let owner = FunctionId::new(1);
        let id = management.acquire_channel(owner).await.unwrap();
        grant.send(id).unwrap();
        prepared.await.unwrap();
        // When: real hardware faults, firmware reports each episode and PF explicitly resets it.
        for episode in 0..if overflow { 3 } else { 1 } {
            trigger.send(()).await.unwrap();
            interrupt.event.wait().await;
            if !overflow {
                assert_eq!(
                    management.try_event().await.unwrap(),
                    Some(ControlEvent {
                        kind: ControlEventKind::DmaFatal,
                        code: 1
                    })
                );
            }
            if overflow && episode == 2 {
                assert_eq!(
                    management.event_error().await.unwrap(),
                    EventRingErrorCode::Overflow
                );
                assert!(matches!(
                    management.try_event().await,
                    Err(Error::EventDelivery(EventRingErrorCode::Overflow))
                ));
            }
            check_closed(&checks).await;
            assert!(matches!(
                management.acquire_channel(owner).await,
                Err(Error::ManagementStatus(ControlResponseStatus::DeviceFailed))
            ));
            reset_dma(&mut management).await;
            // Then: reset preserves the VF allocation but leaves its data access closed.
            check_closed(&checks).await;
            assert!(matches!(
                management.acquire_channel(owner).await,
                Err(Error::ManagementStatus(ControlResponseStatus::NoResources))
            ));
        }
        if overflow {
            recover_events(&mut management, storage.event, &trigger).await;
        }
        drop(checks);
        management.reset_channel(owner, id).await.unwrap();
        resume.send(()).unwrap();
        copied.await.unwrap();
        management.release_channel(owner, id).await.unwrap();
        drop(trigger);
    });
    let program = async {
        let ((), vf, ()) = tokio::join!(pf_program, vf_program, injection);
        assert_eq!(vf, 0);
    };
    tokio::time::timeout(Duration::from_secs(5), system.run_until(Box::pin(program)))
        .await
        .unwrap()
        .unwrap();
}

async fn copy(
    bus: &super::super::HostBus<SharedMemoryAccess<(), MemoryAccessError>>,
    allocator: &SharedMemoryAllocator,
    id: ChannelId,
) {
    let memory = allocator.memory_access();
    let source = allocator.allocate(8, MemoryAlignment::ONE).unwrap();
    let destination = allocator.allocate(8, MemoryAlignment::ONE).unwrap();
    memory
        .write(source, MemoryAlignment::ONE, b"recovery")
        .unwrap();
    let mut channel = Channel::setup(
        bus.clone(),
        memory.clone(),
        ChannelConfig {
            id,
            rings: Rings {
                command: ring::<Command>(allocator).unwrap(),
                event: ring::<Event>(allocator).unwrap(),
            },
        },
    )
    .await
    .unwrap();
    channel.set_irq_enabled(false).await.unwrap();
    channel
        .submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 17,
            copy_length: 8,
            sg_context: 0,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: source.raw(),
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: destination.raw(),
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }))
        .await
        .unwrap();
    let address = allocator
        .allocate(8, MemoryAlignment::try_new(8).unwrap())
        .unwrap();
    let fence = pdma_model_software::dma::CompletionWord(pdma_model_contract::dma::WordWrite {
        target: pdma_model_contract::dma::WordTarget {
            address: address.raw(),
            context: 0,
            width: pdma_model_contract::dma::WordTargetWidth::Bits64,
        },
        value: 17,
    });
    channel.submit(fence.command(17)).await.unwrap();
    while !fence.is_complete(&memory).unwrap() {
        assert_eq!(channel.event().await.unwrap(), None);
        assert_eq!(
            channel.fatal().await.unwrap(),
            pdma_model_contract::dma::ChannelFatalCode::None
        );
        tokio::task::yield_now().await;
    }

    assert_eq!(
        memory
            .read_buffer::<[u8; 8]>(destination, MemoryAlignment::ONE)
            .unwrap(),
        *b"recovery"
    );
}

async fn reset_dma(
    client: &mut Management<
        super::super::HostBus<SharedMemoryAccess<(), MemoryAccessError>>,
        SharedMemoryAccess<(), MemoryAccessError>,
    >,
) {
    let request = client.submit_reset_dma().await.unwrap();
    let result = client.next_response().await.unwrap();
    assert_eq!(
        (result.id, result.status),
        (request.raw(), ControlResponseStatus::Success)
    );
}

async fn check_closed(checks: &mpsc::Sender<oneshot::Sender<()>>) {
    let (confirm, confirmed) = oneshot::channel();
    checks.send(confirm).await.unwrap();
    confirmed.await.unwrap();
}

async fn recover_events(
    management: &mut Management<
        super::super::HostBus<SharedMemoryAccess<(), MemoryAccessError>>,
        SharedMemoryAccess<(), MemoryAccessError>,
    >,
    event: pdma_model_host::RingConfig<ControlEvent>,
    trigger: &mpsc::Sender<()>,
) {
    assert_eq!(
        management.event_error().await.unwrap(),
        EventRingErrorCode::Overflow
    );
    let request = management.reset_events(event).await.unwrap();
    let result = management.next_response().await.unwrap();
    assert_eq!(
        (result.id, result.status),
        (request.raw(), ControlResponseStatus::Success)
    );
    assert_eq!(management.try_event().await.unwrap(), None);
    trigger.send(()).await.unwrap();
    assert_eq!(
        management.next_event().await.unwrap().kind,
        ControlEventKind::DmaFatal
    );
    reset_dma(management).await;
}
