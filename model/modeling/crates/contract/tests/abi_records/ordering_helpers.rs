fn broadcast_source() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::BroadcastSource,
        DmaAddress::new(0x4100),
        site(AbiEventStage::Source, AbiEventAccess::Read, 0),
    )
}

fn broadcast_list() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::BroadcastList,
        DmaAddress::new(0x4200),
        site(AbiEventStage::IndirectList, AbiEventAccess::Read, 0),
    )
}

fn broadcast_destination(index: u64) -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::BroadcastDestination { index },
        DmaAddress::new(0x4300 + index),
        site(AbiEventStage::Destination, AbiEventAccess::Write, index),
    )
}

fn vector_source_a() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::VectorSourceA,
        DmaAddress::new(0x5100),
        site(AbiEventStage::Source, AbiEventAccess::Read, 0),
    )
}

fn vector_source_b() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::VectorSourceB,
        DmaAddress::new(0x5101),
        site(AbiEventStage::Source, AbiEventAccess::Read, 1),
    )
}

fn vector_destination() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::VectorDestination,
        DmaAddress::new(0x5200),
        site(AbiEventStage::Destination, AbiEventAccess::Write, 0),
    )
}

fn arithmetic_start() -> AbiFaultCandidate {
    let visit = AbiOperandVisit::StridedDestination { row: 2 };
    candidate(
        AbiFault::range_overflow_strided(
            DmaAddress::new(u64::MAX - 3),
            AbiStrideOffset::new(2, 8),
            site(AbiEventStage::Destination, AbiEventAccess::Write, 2),
        ),
        AbiFaultOrder::Operand {
            visit,
            step: AbiOperandFaultStep::CheckedStart,
        },
    )
}

fn arithmetic_end() -> AbiFaultCandidate {
    let visit = AbiOperandVisit::StridedDestination { row: 2 };
    candidate(
        AbiFault::range_overflow_direct(
            DmaAddress::new(u64::MAX - 7),
            16,
            site(AbiEventStage::Destination, AbiEventAccess::Write, 2),
        ),
        AbiFaultOrder::Operand {
            visit,
            step: AbiOperandFaultStep::CheckedEnd,
        },
    )
}

fn arithmetic_unmapped() -> AbiFaultCandidate {
    operand_unmapped(
        AbiOperandVisit::StridedDestination { row: 2 },
        DmaAddress::new(0x6000),
        site(AbiEventStage::Destination, AbiEventAccess::Write, 2),
    )
}

fn arithmetic_out_of_bounds() -> AbiFaultCandidate {
    let visit = AbiOperandVisit::StridedDestination { row: 2 };
    candidate(
        AbiFault::out_of_bounds(
            DmaAddress::new(0x6030),
            AbiBounds::new(0x30, 0x6040),
            site(AbiEventStage::Destination, AbiEventAccess::Write, 2),
        ),
        AbiFaultOrder::Operand {
            visit,
            step: AbiOperandFaultStep::OutOfBounds,
        },
    )
}
