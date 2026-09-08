use crate::DeviceCore;
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::abi::mmio::{MmioAccess, PdmaMmioMap, decode_read, register_at};
use pdma_model_contract::semantic::ResourceConfig;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub(super) fn typed_values(registers: &DeviceCore) -> [u64; 66] {
    let global = &registers.global_registers;
    let queue = &registers.queues.registers;
    let irq = &registers.irq.registers;
    [
        u64::from(global.magic.raw()),
        u64::from(global.mmio_abi_version.raw()),
        u64::from(global.req_abi_version.raw()),
        global.req_features.raw(),
        global.reset_expected_epoch.raw(),
        u64::from(global.command.raw()),
        u64::from(global.result.raw()),
        global.detail0.raw(),
        global.detail1.raw(),
        global.detail2.raw(),
        u64::from(global.response.abi_version.raw()),
        u64::from(global.response.run_state.raw()),
        global.response.features.raw(),
        global.response.epoch.raw(),
        u64::from(global.response.max_ring_depth.raw()),
        u64::from(global.response.max_indirect_destinations.raw()),
        u64::from(global.response.max_transfer_bytes.raw()),
        u64::from(global.response.max_strided_rows.raw()),
        u64::from(global.response.max_stride.raw()),
        queue.expected_epoch.raw(),
        queue.id.raw(),
        queue.expected_cursor.raw(),
        queue.new_cursor.raw(),
        u64::from(queue.cfg_sq_depth.raw()),
        u64::from(queue.cfg_cq_depth.raw()),
        u64::from(queue.cfg_event_policy.raw()),
        queue.cfg_software_owner.raw(),
        queue.sq_base.raw(),
        u64::from(queue.sq_depth.raw()),
        queue.cq_base.raw(),
        u64::from(queue.cq_depth.raw()),
        queue.eq_base.raw(),
        u64::from(queue.eq_depth.raw()),
        u64::from(queue.command.raw()),
        u64::from(queue.result.raw()),
        queue.detail0.raw(),
        queue.detail1.raw(),
        queue.detail2.raw(),
        queue.response.epoch.raw(),
        queue.response.queue_id.raw(),
        queue.response.sq_producer.raw(),
        queue.response.sq_captured.raw(),
        queue.response.sq_consumer.raw(),
        queue.response.cq_producer.raw(),
        queue.response.cq_consumer.raw(),
        queue.response.eq_producer.raw(),
        queue.response.eq_consumer.raw(),
        irq.expected_epoch.raw(),
        irq.source_id.raw(),
        u64::from(irq.source_kind.raw()),
        u64::from(irq.route_kind.raw()),
        irq.route_vector.raw(),
        u64::from(irq.command.raw()),
        u64::from(irq.result.raw()),
        irq.detail0.raw(),
        irq.detail1.raw(),
        irq.detail2.raw(),
        irq.response.epoch.raw(),
        irq.response.source_id.raw(),
        u64::from(irq.response.source_kind.raw()),
        u64::from(irq.response.state.raw()),
        u64::from(irq.response.route_kind.raw()),
        u64::from(irq.response.notification_kind.raw()),
        irq.response.vector.raw(),
        irq.response.message_id.raw(),
        irq.response.pending_messages.raw(),
    ]
}
fn write_typed_requests(registers: &mut DeviceCore) {
    let global = &mut registers.global_registers;
    let queue = &mut registers.queues.registers;
    let irq = &mut registers.irq.registers;
    global.req_abi_version = 2.into();
    global.req_features = 2.into();
    global.reset_expected_epoch = 2.into();
    queue.expected_epoch = 2.into();
    queue.id = 2.into();
    queue.expected_cursor = 2.into();
    queue.new_cursor = 2.into();
    queue.cfg_sq_depth = 2.into();
    queue.cfg_cq_depth = 2.into();
    queue.cfg_event_policy = 0.into();
    queue.cfg_software_owner = 2.into();
    queue.sq_base = 2.into();
    queue.sq_depth = 2.into();
    queue.cq_base = 2.into();
    queue.cq_depth = 2.into();
    queue.eq_base = 2.into();
    queue.eq_depth = 2.into();
    irq.expected_epoch = 2.into();
    irq.source_id = 2.into();
    irq.source_kind = 1.into();
    irq.route_kind = 1.into();
    irq.route_vector = 2.into();
}

#[test]
fn every_typed_leaf_observes_raw_requests() -> TestResult {
    // Given: the three hardware owners reached by the endpoint.
    let mut registers = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    // When: raw writes set every writable request to its lowest permitted bit.
    for register in PdmaMmioMap::resolve()?.registers() {
        let definition = register.definition();
        if definition.access() == MmioAccess::ReadWrite {
            let value = definition.writable_mask() & 1;
            registers
                .write_mmio_register(register_at(register.offset()).ok_or("register")?, value)
                .ok_or("register write")?;
        }
    }
    // Then: all typed leaves, including command and response leaves, retain exact values.
    assert_eq!(
        typed_values(&registers),
        [
            0x414d_4450,
            1,
            1,
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            1,
            1,
            1,
            1,
            1,
            0,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            1,
            1,
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
        ]
    );
    Ok(())
}

#[test]
fn every_typed_request_updates_raw_bytes() -> TestResult {
    // Given: fresh hardware register owners.
    let mut registers = DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default());
    // When: every request is changed through its owned typed field.
    write_typed_requests(&mut registers);
    // Then: raw leaf bytes contain the independently specified values.
    for (offset, width, value) in [
        (0x008, 4, 2),
        (0x010, 8, 2),
        (0x018, 8, 2),
        (0x100, 8, 2),
        (0x108, 8, 2),
        (0x110, 8, 2),
        (0x118, 8, 2),
        (0x120, 4, 2),
        (0x124, 4, 2),
        (0x128, 4, 0),
        (0x130, 8, 2),
        (0x138, 8, 2),
        (0x140, 4, 2),
        (0x148, 8, 2),
        (0x150, 4, 2),
        (0x158, 8, 2),
        (0x160, 4, 2),
        (0x200, 8, 2),
        (0x208, 8, 2),
        (0x210, 4, 1),
        (0x214, 4, 1),
        (0x218, 8, 2),
    ] {
        let register = decode_read(offset, width)?;
        let bytes = u64::to_le_bytes(value);
        assert_eq!(
            registers.read_register(register).ok_or("register read")?,
            bytes.get(..usize::try_from(width)?).ok_or("width")?
        );
    }
    Ok(())
}
