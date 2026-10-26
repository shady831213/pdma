//! Independent complete-command ABI fixture.
use hardware_interface_schema::{Decode, Encode, RecordLayout, SchemaCodecError};
use pdma_model_contract::dma::*;

#[test]
fn complete_command_has_96_byte_header_first_wire() {
    let mut wire = [0_u8; 96];
    wire[0] = 7;
    wire[8] = 42;
    let decoded = Command::from_bytes(&wire).expect("96-byte Trap must decode");
    assert_eq!(decoded.to_bytes().unwrap().as_slice(), wire);
    assert_eq!(Command::RECORD_SIZE, 96);
}

#[test]
fn every_opcode_uses_one_header_and_rejects_inconsistent_encoding() {
    for opcode in 0_u8..9 {
        let mut wire = [0_u8; 96];
        wire[0] = opcode;
        wire[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        let mut command = Command::from_bytes(&wire).unwrap();
        assert_eq!(command.id(), u64::MAX);
        assert_eq!(command.to_bytes().unwrap(), wire);
        assert_eq!(command.encode().unwrap().decode().unwrap(), command);
        let header = match &mut command {
            Command::Copy(v) => &mut v.header,
            Command::Write(v) => &mut v.header,
            Command::Fill(v) => &mut v.header,
            Command::Wait(v) => &mut v.header,
            Command::Atomic(v) => &mut v.header,
            Command::Semaphore(v) => &mut v.header,
            Command::Fence(v) => &mut v.header,
            Command::Trap(v) => &mut v.header,
            Command::Timestamp(v) => &mut v.header,
        };
        header.opcode = if opcode == 0 {
            Opcode::Trap
        } else {
            Opcode::Copy
        };
        assert!(matches!(
            command.to_bytes(),
            Err(SchemaCodecError::ConstantMismatch { .. })
        ));
        assert!(command.encode().is_err());
        let mut target = [0xa5; 96];
        assert!(<Command as RecordLayout>::encode(&mut target, &command).is_err());
        assert_eq!(target, [0xa5; 96]);
    }
}

#[test]
fn protocol_rejects_short_unknown_and_every_reserved_gap_or_tail_byte() {
    for length in 0..96 {
        assert!(Command::from_bytes([0; 96].get(..length).unwrap()).is_err());
    }
    for opcode in 9_u8..=255 {
        let mut wire = [0; 96];
        wire[0] = opcode;
        assert!(matches!(
            Command::from_bytes(&wire),
            Err(SchemaCodecError::UnknownTag { .. })
        ));
    }
    for (opcode, extent, gaps) in [
        (0, 96, &[2, 3][..]),
        (1, 40, &[1, 2, 3, 4, 5, 6, 7][..]),
        (2, 48, &[1, 2, 3, 4, 5, 6, 7][..]),
        (3, 48, &[1, 2, 3, 4, 5, 6, 7, 44, 45, 46, 47][..]),
        (
            4,
            56,
            &[1, 2, 3, 4, 5, 6, 7, 48, 49, 50, 51, 52, 53, 54, 55][..],
        ),
        (5, 40, &[1, 2, 3, 4, 5, 6, 7, 36, 37, 38, 39][..]),
        (6, 40, &[1, 2, 3, 4, 5, 6, 7][..]),
        (7, 16, &[1, 2, 3, 4, 5, 6, 7][..]),
        (8, 32, &[1, 2, 3, 4, 5, 6, 7, 28, 29, 30, 31][..]),
    ] {
        for index in gaps.iter().copied().chain(extent..96) {
            let mut wire = [0_u8; 96];
            wire[0] = opcode;
            *wire.get_mut(index).unwrap() = 1;
            assert!(
                Command::from_bytes(&wire).is_err(),
                "opcode {opcode}, byte {index}"
            );
        }
    }
    assert_eq!(Command::ZERO, Command::Copy(CopyCommand::ZERO));
    assert_eq!(Command::ZERO.to_bytes().unwrap(), [0; 96]);
}
