//! Composable full-block storage and geometry conformance.

use hardware_interface_schema::{Decode, Encode};
use pdma_model_contract::abi::mmio::{self, GlobalRegistersValues, QueueRegistersValues};

#[test]
fn full_blocks_preserve_typed_fields_and_nested_response_bytes() {
    // Given: independently owned global and queue register values.
    let mut global = GlobalRegistersValues::default();
    let mut queue = QueueRegistersValues::default();
    global.req_features = mmio::global::GReqFeaturesValues::new(0x0102_0304_0506_0708);
    queue.response.epoch = mmio::queue::QRspEpochValues::new(0x1122_3344_5566_7788);

    // When: each complete block is encoded without constructing a root byte image.
    let global_bytes = global.encode().expect("global values encode");
    let queue_bytes = queue.encode().expect("queue values encode");

    // Then: inferred child geometry and generated codecs retain the typed values.
    assert_eq!(
        &global_bytes.as_bytes()[0x10..0x18],
        &0x0102_0304_0506_0708_u64.to_le_bytes()
    );
    assert_eq!(
        &queue_bytes.as_bytes()[0x88..0x90],
        &0x1122_3344_5566_7788_u64.to_le_bytes()
    );
    assert_eq!(global_bytes.decode().unwrap(), global);
    assert_eq!(queue_bytes.decode().unwrap(), queue);
}

#[test]
fn nested_geometry_is_inferred_from_the_declared_register_sequence() {
    // Given: independently composable full-block layout markers.
    use mmio::{GlobalRegisters, IrqRegisters, PdmaMmioMap, QueueRegisters};

    // When: their generated placement constants are inspected.
    let geometry = [
        (
            PdmaMmioMap::GLOBAL.offset(),
            GlobalRegisters::RESPONSE.offset(),
            GlobalRegisters::EXTENT,
        ),
        (
            PdmaMmioMap::QUEUE.offset(),
            QueueRegisters::RESPONSE.offset(),
            QueueRegisters::EXTENT,
        ),
        (
            PdmaMmioMap::IRQ.offset(),
            IrqRegisters::RESPONSE.offset(),
            IrqRegisters::EXTENT,
        ),
    ];

    // Then: mixed-width children naturally pack around their required alignments.
    assert_eq!(
        geometry,
        [(0, 0x40, 0x6c), (0x100, 0x88, 0xd0), (0x200, 0x40, 0x78)]
    );
    assert_eq!(QueueRegisters::COMMAND.offset(), 0x64);
    assert_eq!(QueueRegisters::RESULT.offset(), 0x68);
    assert_eq!(mmio::register_at(0x16c), None);
}
