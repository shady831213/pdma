use pdma_model_contract::abi::mmio::{PdmaMmioMap, PdmaMmioMapView, PdmaMmioMapViewMut};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; 0x1000];
    let registers = PdmaMmioMap::resolve()?;
    let mut view = PdmaMmioMapViewMut::new(&mut bytes)?;
    view.g_req_abi_version_mut()?.set_bits(0x1234_5678)?;
    view.q_cfg_sq_depth_mut()?.set_bits(1)?;
    view.i_rsp_state_mut()?.set_bits(0x40)?;
    let view = PdmaMmioMapView::new(&bytes)?;
    let roundtrip = view.g_req_abi_version()?.bits()? == 0x1234_5678
        && view.q_cfg_sq_depth()?.bits()? == 1
        && view.i_rsp_state()?.bits()? == 0x40;
    println!(
        "registers={};map=0x{:x};global={};queue={};irq={};roundtrip={roundtrip}",
        registers.registers().len(),
        registers.size(),
        hex(&bytes[0x008..0x00c]),
        hex(&bytes[0x120..0x124]),
        hex(&bytes[0x254..0x258]),
    );
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
