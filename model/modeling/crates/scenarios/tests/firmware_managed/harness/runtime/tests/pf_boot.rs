use super::{AccessError, Managed, control::memory::poll_once};
use crate::harness::{
    program::{
        self, Ports,
        functions::{Assignment, Grant},
    },
    runtime::{Host, HostBus, ManagementBus},
};
use hardware_interface_runtime::{
    MemoryAddress, RegisterAccess, SharedMemory, SharedMemoryAccess, SharedMemoryAllocator,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::control::ControlConfiguration;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::hw_components::firmware::{
    FirmwareBootError, FirmwareBootState, FirmwareBootValues,
};
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::Error;
use std::{pin::pin, sync::Arc, task::Poll, time::Duration};
use tokio::sync::oneshot;

fn platform() -> (
    Managed<SharedMemoryAccess>,
    Host<SharedMemoryAccess>,
    SharedMemoryAllocator,
) {
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let (system, mut hosts) = Managed::with_functions(allocator.memory_access(), 2, 3).unwrap();
    (system, hosts.remove(0), allocator)
}

fn assignments() -> ([Assignment; 2], [Grant; 2]) {
    let [(first, a), (second, b)] = [1, 2].map(|function| {
        let (send, channel) = oneshot::channel();
        let (finished, receive) = oneshot::channel();
        (
            Assignment {
                function: FunctionId::new(function),
                channel: send,
                finished: receive,
            },
            Grant { channel, finished },
        )
    });
    ([first, second], [a, b])
}

async fn control_config(bus: &HostBus<SharedMemoryAccess>) -> (u64, u32) {
    let mut bootstrap = [0; 8];
    let mut enable = [0; 4];
    bus.read(bus.addresses().control, &mut bootstrap)
        .await
        .unwrap();
    bus.read(
        bus.addresses().control
            + u32::try_from(
                ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset(),
            )
            .unwrap(),
        &mut enable,
    )
    .await
    .unwrap();
    (u64::from_le_bytes(bootstrap), u32::from_le_bytes(enable))
}

#[tokio::test]
async fn pf_waits_for_delayed_firmware_before_publishing_management() {
    // Given: real hardware remains Booting while PF software starts first.
    let (system, host, allocator) = platform();
    let (assignments, grants) = assignments();
    let mut running = pin!(host.runtime.run_main(async |interrupts| {
        program::functions::manage(
            Ports {
                bus: host.bus.clone(),
                memory: allocator.memory_access(),
                allocator,
                interrupts,
            },
            assignments,
            None,
        )
        .await
    }));
    // When: PF runs before the firmware actor is polled.
    assert!(poll_once(running.as_mut()).await.is_pending());
    // Then: control configuration stays untouched until firmware becomes Ready.
    assert_eq!(control_config(&host.bus).await, (0, 0));
    let (result, channels) = tokio::time::timeout(
        Duration::from_secs(5),
        system.run_until(async {
            tokio::join!(running.as_mut(), async {
                let [a, b] = grants;
                let first = a.channel.await.unwrap();
                let second = b.channel.await.unwrap();
                a.finished.send(()).unwrap();
                b.finished.send(()).unwrap();
                [first, second]
            })
        }),
    )
    .await
    .unwrap()
    .unwrap();
    result.unwrap().0.unwrap();
    assert_eq!(channels, [ChannelId::new(0), ChannelId::new(1)]);
}

#[tokio::test]
async fn pf_reports_real_firmware_boot_failure_without_configuring_management() {
    // Given: real firmware publishes Failed for an invalid target namespace.
    let (system, host, allocator) = platform();
    let bus = system
        .firmware_cpu
        .bind_registers(ManagementBus(Arc::clone(&system.shared)));
    let mut firmware = pin!(system.firmware_runtime.run_main(async |registration| {
        pdma_model_firmware::main(
            bus,
            system.firmware_memory,
            registration,
            pdma_model_firmware::Configuration {
                addresses: host.bus.addresses(),
                function_count: 0,
            },
        )
        .await
    }));
    assert!(poll_once(firmware.as_mut()).await.is_pending());
    let mut status = [0; 4];
    host.bus
        .read(host.bus.addresses().firmware, &mut status)
        .await
        .unwrap();
    assert_eq!(
        FirmwareBootValues::new(u32::from_le_bytes(status))
            .state()
            .unwrap(),
        FirmwareBootState::Failed
    );
    let (assignments, _grants) = assignments();
    let mut running = pin!(host.runtime.run_main(async |interrupts| {
        program::functions::manage(
            Ports {
                bus: host.bus.clone(),
                memory: allocator.memory_access(),
                allocator,
                interrupts,
            },
            assignments,
            None,
        )
        .await
    }));
    // When: PF enters its actual management program after failed firmware boot.
    let error = match poll_once(running.as_mut()).await {
        Poll::Ready(result) => result.unwrap().0.unwrap_err(),
        Poll::Pending => {
            panic!("PF waits for a control response instead of reporting firmware Failed")
        }
    };
    // Then: software returns the typed boot failure without publishing control state.
    assert!(matches!(
        error.downcast_ref::<Error<AccessError>>(),
        Some(Error::FirmwareFailed(FirmwareBootError::WindowSetup))
    ));
    assert_eq!(control_config(&host.bus).await, (0, 0));
}
