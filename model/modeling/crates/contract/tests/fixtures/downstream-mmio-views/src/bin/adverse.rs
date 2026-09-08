use pdma_model_contract::abi::registers::{
    MmioAccessError, RegisterLocation, decode_read, decode_write,
};

fn main() {
    let mut state = [0xa5_u8; 32];
    let before = state;
    let errors = [
        apply(&mut state, decode_write(u32::MAX, &[1])).is_err(),
        decode_read(u32::MAX - 3, 8).is_err(),
        decode_read(65, 8).is_err(),
        decode_read(68, 4).is_err(),
        decode_read(60, 4).is_err(),
        decode_read(64, 4).is_err(),
        decode_read(52, 4).is_err(),
        apply(&mut state, decode_write(108, &0x101_u32.to_le_bytes())).is_err(),
    ];
    assert!(errors.into_iter().all(|rejected| rejected));
    assert_eq!(before, state);
    println!(
        "width,bounds,alignment,exact-start,reserved,register-width,permission,mask;unchanged=true"
    );
}

fn apply(
    state: &mut [u8; 32],
    decoded: Result<(RegisterLocation, u64), MmioAccessError>,
) -> Result<(), MmioAccessError> {
    let (_, value) = decoded?;
    state[..8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
