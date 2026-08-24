fn copy_case() -> ConflictCase {
    case(
        "copy source before destination",
        vec![copy_destination_unmapped(), copy_source_unmapped()],
        (AbiEventKind::UnmappedDmaAddress, 0x2100, 8, 0, 3, 1, 0),
    )
}

fn strided_case() -> ConflictCase {
    case(
        "strided row then source destination",
        vec![
            operand_unmapped(
                AbiOperandVisit::StridedSource { row: 2 },
                DmaAddress::new(0x3200),
                site(AbiEventStage::Source, AbiEventAccess::Read, 2),
            ),
            operand_unmapped(
                AbiOperandVisit::StridedDestination { row: 1 },
                DmaAddress::new(0x3101),
                site(AbiEventStage::Destination, AbiEventAccess::Write, 1),
            ),
            operand_unmapped(
                AbiOperandVisit::StridedSource { row: 1 },
                DmaAddress::new(0x3100),
                site(AbiEventStage::Source, AbiEventAccess::Read, 1),
            ),
        ],
        (AbiEventKind::UnmappedDmaAddress, 0x3100, 8, 0, 3, 1, 1),
    )
}

fn broadcast_source_case() -> ConflictCase {
    case(
        "broadcast source before list destinations",
        vec![
            broadcast_destination(2),
            broadcast_destination(0),
            broadcast_list(),
            broadcast_source(),
        ],
        (AbiEventKind::UnmappedDmaAddress, 0x4100, 8, 0, 3, 1, 0),
    )
}

fn broadcast_list_case() -> ConflictCase {
    case(
        "broadcast list before destinations",
        vec![
            broadcast_destination(2),
            broadcast_destination(0),
            broadcast_list(),
        ],
        (AbiEventKind::UnmappedDmaAddress, 0x4200, 8, 0, 2, 1, 0),
    )
}

fn broadcast_destination_case() -> ConflictCase {
    case(
        "broadcast destinations ascending",
        vec![broadcast_destination(2), broadcast_destination(0)],
        (AbiEventKind::UnmappedDmaAddress, 0x4300, 8, 0, 4, 2, 0),
    )
}

fn vector_source_a_case() -> ConflictCase {
    case(
        "vector source a before b destination",
        vec![
            vector_destination(),
            vector_source_b(),
            vector_source_a(),
        ],
        (AbiEventKind::UnmappedDmaAddress, 0x5100, 8, 0, 3, 1, 0),
    )
}

fn vector_source_b_case() -> ConflictCase {
    case(
        "vector source b before destination",
        vec![vector_destination(), vector_source_b()],
        (AbiEventKind::UnmappedDmaAddress, 0x5101, 8, 0, 3, 1, 1),
    )
}

fn arithmetic_start_case() -> ConflictCase {
    case(
        "checked start before end unmapped bounds",
        vec![
            arithmetic_out_of_bounds(),
            arithmetic_unmapped(),
            arithmetic_end(),
            arithmetic_start(),
        ],
        (
            AbiEventKind::RangeOverflow,
            u64::MAX - 3,
            2,
            8,
            4,
            2,
            2,
        ),
    )
}

fn arithmetic_end_case() -> ConflictCase {
    case(
        "checked end before unmapped bounds",
        vec![
            arithmetic_out_of_bounds(),
            arithmetic_unmapped(),
            arithmetic_end(),
        ],
        (
            AbiEventKind::RangeOverflow,
            u64::MAX - 7,
            16,
            0,
            4,
            2,
            2,
        ),
    )
}

fn access_fault_case() -> ConflictCase {
    case(
        "unmapped before out of bounds",
        vec![arithmetic_out_of_bounds(), arithmetic_unmapped()],
        (AbiEventKind::UnmappedDmaAddress, 0x6000, 8, 0, 4, 2, 2),
    )
}

fn copy_source_unmapped() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::CopySource,
        DmaAddress::new(0x2100),
        site(AbiEventStage::Source, AbiEventAccess::Read, 0),
    )
}

fn copy_destination_unmapped() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::CopyDestination,
        DmaAddress::new(0x2200),
        site(AbiEventStage::Destination, AbiEventAccess::Write, 0),
    )
}

fn operand_unmapped(
    visit: AbiOperandVisit,
    locator: DmaAddress,
    fault_site: AbiFaultSite,
) -> AbiFaultCandidate {
    candidate(
        AbiFault::unmapped(locator, 8, fault_site),
        AbiFaultOrder::Operand {
            visit,
            step: AbiOperandFaultStep::Unmapped,
        },
    )
}
