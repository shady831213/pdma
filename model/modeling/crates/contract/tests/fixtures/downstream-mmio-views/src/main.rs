use pdma_model_contract::abi::registers::{
    PdmaRegisterMap, PdmaRegisterMapView, PdmaRegisterMapViewMut,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; PdmaRegisterMap::EXTENT];
    let registers = PdmaRegisterMap::resolve()?;
    let mut view = PdmaRegisterMapViewMut::new(&mut bytes)?;
    view.queues_mut()?
        .queue_mut()?
        .sq_depth_mut()?
        .set_value(0x1234_5678)?;
    view.queues_mut()?
        .sources_mut(1)?
        .route_vector_mut()?
        .set_value(0x40)?;
    let view = PdmaRegisterMapView::new(&bytes)?;
    let roundtrip = view.queues()?.queue()?.sq_depth()?.value()? == 0x1234_5678
        && view.queues()?.sources(1)?.route_vector()?.value()? == 0x40
        && view.queues()?.sources(0)?.route_vector()?.value()? == 0;
    assert!(roundtrip);
    assert_eq!(&bytes[72..76], &[0x78, 0x56, 0x34, 0x12]);
    assert_eq!(&bytes[224..232], &0x40_u64.to_le_bytes());
    println!(
        "registers={};map=0x{:x};roundtrip={roundtrip}",
        registers.registers().len(),
        registers.size(),
    );
    Ok(())
}
