use pdma_model_contract::abi::mmio::{MmioAccessError, decode_read, decode_write};

fn main() {
    let mut state = [0xa5_u8; 32];
    let before = state;
    let errors = [
        apply(&mut state, decode_write(0xfff, &[1])).is_err(),
        decode_read(0xffc, 8).is_err(),
        decode_read(0x011, 8).is_err(),
        decode_read(0x02c, 4).is_err(),
        decode_read(0x00c, 4).is_err(),
        decode_read(0x010, 4).is_err(),
        decode_read(0x020, 4).is_err(),
        apply(&mut state, decode_write(0x008, &[0, 0, 1, 0])).is_err(),
    ];
    let categories = [
        "width",
        "bounds",
        "alignment",
        "exact-start",
        "reserved",
        "register-width",
        "permission",
        "mask",
    ];
    let winners = categories
        .into_iter()
        .zip(errors)
        .filter_map(|(category, won)| won.then_some(category))
        .collect::<Vec<_>>()
        .join(",");
    println!("{winners};unchanged={}", before == state);
}

fn apply(
    state: &mut [u8; 32],
    decoded: Result<(pdma_model_contract::abi::mmio::RegisterId, u64), MmioAccessError>,
) -> Result<(), MmioAccessError> {
    let (_, value) = decoded?;
    state[..8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
