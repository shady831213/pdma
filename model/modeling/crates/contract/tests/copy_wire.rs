//! Full-width positions now belong directly to independently selected Copy endpoints.
use pdma_model_contract::dma::*;

#[test]
fn all_copy_modes_preserve_full_width_geometry_and_positions() {
    for mode in 0_u8..4 {
        let mut wire = [0_u8; 96];
        wire[1] = mode;
        wire[4..32].fill(0xff);
        wire[32..48].fill(0xff);
        wire[64..80].fill(0xff);
        if mode & 1 != 0 {
            wire[48..64].fill(0xff);
        }
        if mode & 2 != 0 {
            wire[80..96].fill(0xff);
        }
        let command = Command::from_bytes(&wire).unwrap();
        assert_eq!(command.to_bytes().unwrap(), wire);
        let Command::Copy(copy) = command else {
            panic!("Copy expected")
        };
        assert_eq!(copy.id, u64::MAX);
        assert_eq!(copy.copy_length, u64::MAX);
        assert_eq!(
            (
                copy.sg_context,
                copy.source_context,
                copy.destination_context
            ),
            (u32::MAX, u32::MAX, u32::MAX)
        );
        match copy.source {
            CopyCommandSourceValues::Linear(v) => {
                assert_eq!((v.address, v.offset), (u64::MAX, u64::MAX));
            }
            CopyCommandSourceValues::ScatterGather(v) => assert_eq!(
                (v.list_address, v.count, v.index, v.offset),
                (u64::MAX, u64::MAX, u64::MAX, u64::MAX)
            ),
        }
        match copy.destination {
            CopyCommandDestinationValues::Linear(v) => {
                assert_eq!((v.address, v.offset), (u64::MAX, u64::MAX));
            }
            CopyCommandDestinationValues::ScatterGather(v) => assert_eq!(
                (v.list_address, v.count, v.index, v.offset),
                (u64::MAX, u64::MAX, u64::MAX, u64::MAX)
            ),
        }
    }
}

#[test]
fn inactive_endpoint_words_and_selector_reserved_bits_are_rejected() {
    for index in [1, 2, 3, 48, 56, 80, 88] {
        let mut wire = [0_u8; 96];
        *wire.get_mut(index).unwrap() = if index == 1 { 4 } else { 1 };
        assert!(Command::from_bytes(&wire).is_err(), "byte {index}");
    }
}
