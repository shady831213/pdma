fn canonical_events() -> [AbiEvent; 11] {
    let queue = 0x1122;
    let command = 0x3344;
    let descriptor = DmaAddress::new(0x1000);
    [
        AbiEvent::new(
            queue,
            command,
            AbiFault::unmapped(
                DmaAddress::new(0x2000),
                0x20,
                site(AbiEventStage::Source, AbiEventAccess::Read, 0),
            ),
        ),
        AbiEvent::new(
            queue,
            command,
            AbiFault::range_overflow_direct(
                DmaAddress::new(u64::MAX - 7),
                16,
                site(AbiEventStage::Destination, AbiEventAccess::Write, 0),
            ),
        ),
        AbiEvent::new(
            queue,
            command,
            AbiFault::out_of_bounds(
                DmaAddress::new(0x3030),
                AbiBounds::new(0x30, 0x3040),
                site(AbiEventStage::Destination, AbiEventAccess::Write, 0),
            ),
        ),
        AbiEvent::new(queue, command, AbiFault::invalid_version(descriptor, 2)),
        AbiEvent::new(queue, command, AbiFault::invalid_size(descriptor, 63)),
        AbiEvent::new(queue, command, AbiFault::invalid_opcode(descriptor, 9)),
        AbiEvent::new(queue, command, AbiFault::invalid_flags(descriptor, 1)),
        AbiEvent::new(
            queue,
            command,
            AbiFault::invalid_reserved(descriptor, 40, 1),
        ),
        AbiEvent::new(
            queue,
            command,
            AbiFault::invalid_indirect_count(DmaAddress::new(0x4000), 0),
        ),
        AbiEvent::new(
            queue,
            command,
            AbiFault::invalid_indirect_alignment(DmaAddress::new(0x4003)),
        ),
        AbiEvent::new(
            queue,
            command,
            AbiFault::value_out_of_range(descriptor, 32, u64::from(u32::MAX) + 1),
        ),
    ]
}

fn event_golden(tuple: (u16, u64, u64, u64, u16, u16, u64)) -> [u8; 64] {
    let mut bytes = [0_u8; 64];
    bytes[0..2].copy_from_slice(&1_u16.to_le_bytes());
    bytes[2..4].copy_from_slice(&64_u16.to_le_bytes());
    bytes[4..6].copy_from_slice(&tuple.0.to_le_bytes());
    bytes[8..16].copy_from_slice(&0x1122_u64.to_le_bytes());
    bytes[16..24].copy_from_slice(&0x3344_u64.to_le_bytes());
    bytes[24..32].copy_from_slice(&tuple.1.to_le_bytes());
    bytes[32..40].copy_from_slice(&tuple.2.to_le_bytes());
    bytes[40..48].copy_from_slice(&tuple.3.to_le_bytes());
    bytes[48..50].copy_from_slice(&tuple.4.to_le_bytes());
    bytes[50..52].copy_from_slice(&tuple.5.to_le_bytes());
    bytes[56..64].copy_from_slice(&tuple.6.to_le_bytes());
    bytes
}

const fn noncanonical_memory_events() -> [AbiEvent; 3] {
    [
        invalid_event(
            AbiEventKind::UnmappedDmaAddress,
            AbiFaultDetails::new(8, 1, site(AbiEventStage::Source, AbiEventAccess::Read, 0)),
        ),
        invalid_event(
            AbiEventKind::RangeOverflow,
            AbiFaultDetails::new(1, 0, site(AbiEventStage::Source, AbiEventAccess::Read, 0)),
        ),
        invalid_event(
            AbiEventKind::OutOfBounds,
            AbiFaultDetails::new(
                8,
                0x1000,
                site(AbiEventStage::Destination, AbiEventAccess::Write, 0),
            ),
        ),
    ]
}

fn noncanonical_descriptor_events() -> [AbiEvent; 8] {
    [
        invalid_event(
            AbiEventKind::InvalidVersion,
            AbiFaultDetails::new(1, 1, site(AbiEventStage::Descriptor, AbiEventAccess::None, 0)),
        ),
        invalid_event(
            AbiEventKind::InvalidSize,
            AbiFaultDetails::new(64, 64, site(AbiEventStage::Descriptor, AbiEventAccess::None, 6)),
        ),
        invalid_event(
            AbiEventKind::InvalidOpcode,
            AbiFaultDetails::new(1, 4, site(AbiEventStage::Descriptor, AbiEventAccess::None, 2)),
        ),
        invalid_event(
            AbiEventKind::InvalidFlags,
            AbiFaultDetails::new(0, 0, site(AbiEventStage::Descriptor, AbiEventAccess::None, 4)),
        ),
        invalid_event(
            AbiEventKind::InvalidReserved,
            AbiFaultDetails::new(0, 0, site(AbiEventStage::Descriptor, AbiEventAccess::None, 40)),
        ),
        invalid_event(
            AbiEventKind::InvalidIndirectCount,
            AbiFaultDetails::new(
                1,
                u64::from(u32::MAX),
                site(AbiEventStage::IndirectList, AbiEventAccess::None, 0),
            ),
        ),
        invalid_event(
            AbiEventKind::InvalidIndirectAlignment,
            AbiFaultDetails::new(0, 8, site(AbiEventStage::IndirectList, AbiEventAccess::None, 0)),
        ),
        invalid_event(
            AbiEventKind::ValueOutOfRange,
            AbiFaultDetails::new(
                u64::from(u32::MAX),
                u64::from(u32::MAX),
                site(AbiEventStage::Descriptor, AbiEventAccess::None, 32),
            ),
        ),
    ]
}

const fn invalid_event(kind: AbiEventKind, details: AbiFaultDetails) -> AbiEvent {
    AbiEvent::new(
        1,
        2,
        AbiFault::from_parts(kind, DmaAddress::new(0x1000), details),
    )
}

const fn site(stage: AbiEventStage, access: AbiEventAccess, index: u64) -> AbiFaultSite {
    AbiFaultSite::new(stage, access, index)
}
