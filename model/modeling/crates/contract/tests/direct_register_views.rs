//! Public generated direct-register values and borrowed views.

use hardware_interface_schema::{Decode, Encode, SchemaCodecError};
use pdma_model_contract::abi::registers::{
    PdmaRegisterMap, PdmaRegisterMapValues, PdmaRegisterMapView, PdmaRegisterMapViewMut, global,
    queue::QueueRegistersViewMut,
};

#[test]
fn owned_composition_encodes_nested_registers_in_little_endian() {
    // Given
    let mut values = PdmaRegisterMapValues::default();
    values.global.epoch = global::GEpochValues::new(0x0102_0304_0506_0708);
    values.queues.queue.sq_base =
        pdma_model_contract::abi::registers::queue::QSqBaseValues::new(0x1112_1314_1516_1718);
    values.queues.sources[1].route_vector =
        pdma_model_contract::abi::registers::irq::IRouteVectorValues::new(0x2122_2324_2526_2728);

    // When
    let raw = values.encode().expect("generated values encode");
    let view = PdmaRegisterMapView::new(raw.as_bytes()).expect("full image");

    // Then
    assert_eq!(
        &raw.as_bytes()[16..24],
        &0x0102_0304_0506_0708_u64.to_le_bytes()
    );
    assert_eq!(
        &raw.as_bytes()[64..72],
        &0x1112_1314_1516_1718_u64.to_le_bytes()
    );
    assert_eq!(
        &raw.as_bytes()[224..232],
        &0x2122_2324_2526_2728_u64.to_le_bytes()
    );
    assert_eq!(
        view.global().unwrap().epoch().unwrap().value(),
        Ok(0x0102_0304_0506_0708)
    );
    assert_eq!(raw.decode(), Ok(values));
}

#[test]
fn root_and_queue_views_alias_without_touching_other_registers()
-> Result<(), Box<dyn std::error::Error>> {
    // Given
    let mut bytes = [0_u8; PdmaRegisterMap::EXTENT];

    // When
    PdmaRegisterMapViewMut::new(&mut bytes)?
        .queues_mut()?
        .queue_mut()?
        .sq_depth_mut()?
        .set_value(0x1122_3344)?;
    QueueRegistersViewMut::new(&mut bytes[64..176])?
        .sq_depth_mut()?
        .set_value(0x5566_7788)?;
    PdmaRegisterMapViewMut::new(&mut bytes)?
        .queues_mut()?
        .sources_mut(1)?
        .route_vector_mut()?
        .set_value(71)?;

    // Then
    let view = PdmaRegisterMapView::new(&bytes)?;
    assert_eq!(view.queues()?.queue()?.sq_depth()?.value()?, 0x5566_7788);
    assert_eq!(view.queues()?.sources(1)?.route_vector()?.value()?, 71);
    assert_eq!(view.queues()?.sources(0)?.route_vector()?.value()?, 0);
    assert_eq!(&bytes[72..76], &[0x88, 0x77, 0x66, 0x55]);
    assert_eq!(view.global()?.epoch()?.value()?, 0);
    Ok(())
}

#[test]
fn short_root_views_preserve_the_supplied_bytes() {
    // Given
    let mut bytes = [0xa5_u8; PdmaRegisterMap::EXTENT - 1];
    let before = bytes;

    // When
    let error = PdmaRegisterMapViewMut::new(&mut bytes).err();

    // Then
    assert_eq!(
        error,
        Some(SchemaCodecError::BufferTooShort {
            required: 256,
            actual: 255
        })
    );
    assert_eq!(bytes, before);
}
