//! Public generated MMIO map and value-view integration.

use pdma_model_contract::abi::mmio::{PdmaMmioMap, PdmaMmioMapView, PdmaMmioMapViewMut};

#[test]
fn generated_map_views_round_trip_global_queue_and_irq_registers()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one exact-size PDMA MMIO image and the public generated map graph.
    let mut bytes = [0_u8; 0x1000];
    let resolved = PdmaMmioMap::resolve()?;
    assert_eq!(resolved.registers().len(), 66);

    // When: representative global, queue, and IRQ registers are written through generated views.
    let mut view = PdmaMmioMapViewMut::new(&mut bytes)?;
    view.global_mut()?
        .req_abi_version_mut()?
        .set_value(0x1234_5678)?;
    view.queue_mut()?.cfg_event_policy_mut()?.set_value(1)?;
    view.irq_mut()?.route_vector_mut()?.set_value(0x40)?;

    // Then: public immutable views recover the exact little-endian values and bytes.
    let view = PdmaMmioMapView::new(&bytes)?;
    assert_eq!(view.global()?.req_abi_version()?.value()?, 0x1234_5678);
    assert_eq!(view.queue()?.cfg_event_policy()?.value()?, 1);
    assert_eq!(view.irq()?.route_vector()?.value()?, 0x40);
    assert_eq!(&bytes[0x008..0x00c], &[0x78, 0x56, 0x34, 0x12]);
    assert_eq!(&bytes[0x128..0x12c], &[1, 0, 0, 0]);
    assert_eq!(&bytes[0x218..0x220], &[0x40, 0, 0, 0, 0, 0, 0, 0]);
    Ok(())
}
