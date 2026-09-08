//! Exact numeric MMIO ABI conformance.

pub mod register_literals;
mod response_banks;
mod schema_parity;

use pdma_model_contract::abi::mmio::{
    GlobalCommand, IrqCommand, IrqSourceStatus, MmioAccess, MmioAccessError, MmioReset, MmioResult,
    QueueCommand, ResponseUpdate, apply_response_update, decode_read, decode_write, encode_u32,
    encode_u64, register_at,
};

#[test]
fn command_and_result_numbers_are_exact() {
    assert_eq!(
        [
            access_code(MmioAccess::ReadOnly),
            access_code(MmioAccess::ReadWrite),
            access_code(MmioAccess::WriteOnly)
        ],
        [0, 1, 2]
    );
    assert_eq!(
        [
            reset_code(MmioReset::Constant),
            reset_code(MmioReset::Zero),
            reset_code(MmioReset::PostReset)
        ],
        [0, 1, 2]
    );
    assert_eq!(
        [
            GlobalCommand::Discover.code(),
            GlobalCommand::ReadLimits.code(),
            GlobalCommand::ReadStatus.code(),
            GlobalCommand::Reset.code()
        ],
        [1, 2, 3, 4]
    );
    assert_eq!(
        [
            QueueCommand::Activate.code(),
            QueueCommand::ReadStatus.code(),
            QueueCommand::PublishSqProducer.code(),
            QueueCommand::CaptureSqProducer.code(),
            QueueCommand::PublishSqConsumer.code(),
            QueueCommand::PublishCqProducer.code(),
            QueueCommand::PublishCqConsumer.code(),
            QueueCommand::PublishEqProducer.code(),
            QueueCommand::PublishEqConsumer.code()
        ],
        [1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(
        [
            IrqCommand::ConfigureRoute.code(),
            IrqCommand::Enable.code(),
            IrqCommand::Disable.code(),
            IrqCommand::Mask.code(),
            IrqCommand::Unmask.code(),
            IrqCommand::Acknowledge.code(),
            IrqCommand::ObserveSource.code(),
            IrqCommand::SnapshotLine.code(),
            IrqCommand::ClaimNotification.code()
        ],
        [1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    let codes = [
        0, 1, 2, 0x100, 0x101, 0x102, 0x103, 0x104, 0x105, 0x106, 0x107, 0x108, 0x109, 0x10a,
        0x10b, 0x10c, 0x10d, 0x10e, 0x10f, 0x110, 0x111, 0x112, 0x113, 0x114, 0x115, 0x116, 0x117,
        0x118, 0x119, 0x11a, 0x11b, 0x11c,
    ];
    assert_eq!(MmioResult::ALL.map(MmioResult::code), codes);
    println!("literal MMIO results checked: {}", codes.len());
}

const fn access_code(access: MmioAccess) -> u32 {
    match access {
        MmioAccess::ReadOnly => 0,
        MmioAccess::ReadWrite => 1,
        MmioAccess::WriteOnly => 2,
    }
}

const fn reset_code(reset: MmioReset) -> u32 {
    match reset {
        MmioReset::Constant => 0,
        MmioReset::Zero => 1,
        MmioReset::PostReset => 2,
    }
}

#[test]
fn codecs_are_exact_little_endian() {
    assert_eq!(encode_u32(0x89ab_cdef), [0xef, 0xcd, 0xab, 0x89]);
    assert_eq!(
        encode_u64(0x0123_4567_89ab_cdef),
        [0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01]
    );
    assert_eq!(
        decode_write(0x120, &[0xef, 0xcd, 0xab, 0x89]),
        Ok((register_at(0x120).unwrap(), 0x89ab_cdef))
    );
    assert_eq!(
        decode_write(0x010, &[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01]),
        Ok((register_at(0x010).unwrap(), 0x0123_4567_89ab_cdef))
    );
    println!("LE32=efcdab89 LE64=efcdab8967452301");
}

#[test]
fn malformed_accesses_follow_closed_precedence() {
    assert_eq!(
        decode_read(0, 1),
        Err(MmioAccessError::WrongWidth {
            offset: 0,
            expected: 0,
            actual: 1
        })
    );
    assert_eq!(
        decode_read(0, 2),
        Err(MmioAccessError::WrongWidth {
            offset: 0,
            expected: 0,
            actual: 2
        })
    );
    assert_eq!(
        decode_read(0, 16),
        Err(MmioAccessError::WrongWidth {
            offset: 0,
            expected: 0,
            actual: 16
        })
    );
    assert_eq!(
        decode_read(0, 256),
        Err(MmioAccessError::WrongWidth {
            offset: 0,
            expected: 0,
            actual: 256
        })
    );
    assert_eq!(
        decode_write(0, &[0; 256]),
        Err(MmioAccessError::WrongWidth {
            offset: 0,
            expected: 0,
            actual: 256
        })
    );
    assert_eq!(
        decode_read(0xffc, 8),
        Err(MmioAccessError::UnmappedOffset { offset: 0xffc })
    );
    assert_eq!(
        decode_read(0x00c, 4),
        Err(MmioAccessError::ReservedOffset { offset: 0x00c })
    );
    assert_eq!(
        decode_read(0x011, 8),
        Err(MmioAccessError::Misaligned {
            offset: 0x011,
            width: 8
        })
    );
    assert_eq!(
        decode_read(0x010, 4),
        Err(MmioAccessError::WrongWidth {
            offset: 0x010,
            expected: 8,
            actual: 4
        })
    );
    assert_eq!(
        decode_read(0x020, 4),
        Err(MmioAccessError::ReadFromWriteOnly { offset: 0x020 })
    );
    assert_eq!(
        decode_write(0x000, &[0; 4]),
        Err(MmioAccessError::WriteToReadOnly { offset: 0 })
    );
    assert_eq!(
        decode_write(0x008, &[0, 0, 1, 0]),
        Err(MmioAccessError::ReservedBits {
            offset: 0x008,
            bits: 0x10000
        })
    );
    assert_eq!(
        decode_write(0x010, &[0; 4]),
        Err(MmioAccessError::WrongWidth {
            offset: 0x010,
            expected: 8,
            actual: 4
        })
    );
    println!("malformed widths include 1,2,16,256 and split64");
}

#[test]
fn response_and_irq_status_rules_are_exact() {
    let previous = [7_u64; 9];
    assert_eq!(
        apply_response_update(previous, ResponseUpdate::Replace([3; 9])),
        [3; 9]
    );
    assert_eq!(
        apply_response_update(previous, ResponseUpdate::Empty),
        [0; 9]
    );
    assert_eq!(
        apply_response_update(previous, ResponseUpdate::Preserve),
        previous
    );
    assert_eq!(
        IrqSourceStatus::try_from(0x3b).map(IrqSourceStatus::bits),
        Ok(0x3b)
    );
    assert_eq!(
        IrqSourceStatus::try_from(0x7f).map(IrqSourceStatus::bits),
        Ok(0x7f)
    );
    assert!(IrqSourceStatus::try_from(0x80).is_err());
    println!("response replace/zero-fill/preserve and IRQ state mask=0x7f checked");
}
