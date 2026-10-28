use pdma_model_contract::dma::{
    ChannelId, Command, CommandHeader, CopyCommandDestinationValues, CopyCommandSourceValues,
    Event, LinearEndpoint, Opcode, TrapCommand,
};

use super::super::{Driver, HostVector, ManagementBus};
use super::{Managed, control::memory::poll_once};
use hw_model_io::memory::{MemoryAccess, MemoryAlignment};
use hw_model_io::memory::{SharedMemory, SharedMemoryAllocator};
use hw_model_io::register::RegisterAccess;
use hw_model_pipeline::{AsyncExecutor, HardwareDriver};
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_host::{Channel, ChannelConfig, Rings};
use pdma_model_software::{
    control::{ControlIsr, InterruptEvent},
    dma::ChannelControl,
};
use std::{pin::pin, sync::Arc, time::Duration};

#[tokio::test]
async fn pending_dma_notification_keeps_old_destination_after_reset_and_regrant() {
    // Given: a real DMA command whose owner has not yet started interrupt dispatch.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), 0x1000.into(), 64 * 1024)
            .expect("valid test heap");
    let memory = allocator.memory_access();
    let (system, mut hosts) = Managed::with_functions(memory.clone(), 1, 2).unwrap();
    let new = hosts.pop().unwrap();
    let old = hosts.pop().unwrap();
    let admin = ManagementBus(Arc::clone(&system.shared));
    let window = old.bus.addresses().windows;
    admin.write(window, &1_u32.to_le_bytes()).await.unwrap();
    let command = super::control::ring::<Command>(&allocator).unwrap();
    let response = super::control::ring::<Event>(&allocator).unwrap();
    let descriptor = payload(&allocator);
    let id = ChannelId::new(0);
    let mut channel = Channel::setup(
        old.bus.clone(),
        memory,
        ChannelConfig {
            id,
            rings: Rings {
                command,
                event: response,
            },
        },
    )
    .await
    .unwrap();
    channel.submit(descriptor).await.unwrap();
    channel
        .submit(Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 2,
        }))
        .await
        .unwrap();
    let hardware = async {
        let executor = AsyncExecutor::new(Driver(Arc::clone(&system.shared)));
        let mut hardware = system.hardware;
        executor
            .drive(hardware.as_mut(), &mut HardwareDriver::new())
            .await
    };
    let program = async {
        while !channel.irq_cause().await.unwrap().trap() {
            tokio::task::yield_now().await;
        }
        drop(channel);
        // When: management drains and reassigns the channel before the old ISR runs.
        admin.write(window, &0_u32.to_le_bytes()).await.unwrap();
        ChannelControl::discover(&admin, id)
            .await
            .unwrap()
            .reset(&admin)
            .await
            .unwrap();
        admin
            .write(
                window + u32::try_from(WindowControl::OWNER.offset()).unwrap(),
                &1_u32.to_le_bytes(),
            )
            .await
            .unwrap();
        admin.write(window, &1_u32.to_le_bytes()).await.unwrap();
        // Then: only the original destination receives the already emitted message.
        let ((), summary) = old
            .runtime
            .run_main::<ControlIsr, _>(async |registration| {
                let event = InterruptEvent::new();
                registration
                    .register(HostVector::Dma, event.handler())
                    .unwrap();
                event.wait().await;
            })
            .await
            .unwrap();
        assert_eq!(summary.deliveries(), 1);
        let ((), summary) = new
            .runtime
            .run_main::<ControlIsr, _>(async |registration| {
                let event = InterruptEvent::new();
                registration
                    .register(HostVector::Dma, event.handler())
                    .unwrap();
                let mut wait = pin!(event.wait());
                assert!(poll_once(wait.as_mut()).await.is_pending());
            })
            .await
            .unwrap();
        assert_eq!(summary.deliveries(), 0);
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = hardware => panic!("hardware unexpectedly terminated: {result:?}"),
            () = program => {},
        }
    })
    .await
    .unwrap();
}

fn payload(allocator: &SharedMemoryAllocator) -> Command {
    let source = allocator.allocate(4, MemoryAlignment::ONE).unwrap();
    let destination = allocator.allocate(4, MemoryAlignment::ONE).unwrap();
    allocator
        .memory_access()
        .write(source, MemoryAlignment::ONE, b"data")
        .unwrap();
    Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 1,
        copy_length: 4,
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
    })
}
