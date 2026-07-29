type FaultTuple = (AbiEventKind, u64, u64, u64, u16, u16, u64);

struct ConflictCase {
    name: &'static str,
    candidates: Vec<AbiFaultCandidate>,
    expected: FaultTuple,
}

#[test]
fn conflicting_faults_have_one_canonical_order_and_auxiliary_tuple() {
    // Given: simultaneous candidates are deliberately reversed from canonical order.
    let cases = conflict_cases();

    // When: the reusable selector evaluates each unsorted candidate set.
    let selected = cases
        .iter()
        .map(|case| {
            select_canonical_fault(case.candidates.iter().copied())
                .expect("candidate set must be compatible")
                .expect("candidate set must not be empty")
                .tuple()
        })
        .collect::<Vec<_>>();

    // Then: every class, field, visit, and within-operand precedence is deterministic.
    for (case, actual) in cases.iter().zip(selected) {
        assert_eq!(actual, case.expected, "case {}", case.name);
        println!("selected {}: {actual:?}", case.name);
    }
}

fn conflict_cases() -> Vec<ConflictCase> {
    vec![
        class_case(),
        header_case(
            "version before size opcode flags",
            &[
                AbiHeaderField::Flags,
                AbiHeaderField::Opcode,
                AbiHeaderField::Size,
                AbiHeaderField::Version,
            ],
            (AbiEventKind::InvalidVersion, 0x1000, 9, 1, 1, 0, 0),
        ),
        header_case(
            "size before opcode flags",
            &[
                AbiHeaderField::Flags,
                AbiHeaderField::Opcode,
                AbiHeaderField::Size,
            ],
            (AbiEventKind::InvalidSize, 0x1000, 63, 64, 1, 0, 6),
        ),
        header_case(
            "opcode before flags",
            &[AbiHeaderField::Flags, AbiHeaderField::Opcode],
            (AbiEventKind::InvalidOpcode, 0x1000, 9, 4, 1, 0, 2),
        ),
        reserved_case(),
        scalar_case(),
        copy_case(),
        strided_case(),
        broadcast_source_case(),
        broadcast_list_case(),
        broadcast_destination_case(),
        vector_source_a_case(),
        vector_source_b_case(),
        arithmetic_start_case(),
        arithmetic_end_case(),
        access_fault_case(),
    ]
}

fn class_case() -> ConflictCase {
    case(
        "header before reserved scalar operand",
        vec![
            copy_source_unmapped(),
            scalar(AbiScalarField::CopyLength, 32),
            reserved(AbiReservedField::CopyOffset40, 40),
            header(AbiHeaderField::Flags),
        ],
        (AbiEventKind::InvalidFlags, 0x1000, 1, 0, 1, 0, 4),
    )
}

fn header_case(
    name: &'static str,
    fields: &[AbiHeaderField],
    expected: FaultTuple,
) -> ConflictCase {
    case(
        name,
        fields.iter().copied().map(header).collect(),
        expected,
    )
}

fn reserved_case() -> ConflictCase {
    case(
        "reserved ascending offset",
        vec![
            reserved(AbiReservedField::CopyOffset56, 56),
            reserved(AbiReservedField::CopyOffset48, 48),
            reserved(AbiReservedField::CopyOffset40, 40),
        ],
        (AbiEventKind::InvalidReserved, 0x1000, 7, 0, 1, 0, 40),
    )
}

fn scalar_case() -> ConflictCase {
    case(
        "scalar ascending payload offset",
        vec![
            scalar(AbiScalarField::StridedDestinationStride, 56),
            scalar(AbiScalarField::StridedSourceStride, 48),
            scalar(AbiScalarField::StridedRowBytes, 40),
            scalar(AbiScalarField::StridedRows, 32),
        ],
        (
            AbiEventKind::ValueOutOfRange,
            0x1000,
            u64::from(u32::MAX) + 1,
            u64::from(u32::MAX),
            1,
            0,
            32,
        ),
    )
}

const fn case(
    name: &'static str,
    candidates: Vec<AbiFaultCandidate>,
    expected: FaultTuple,
) -> ConflictCase {
    ConflictCase {
        name,
        candidates,
        expected,
    }
}

fn header(field: AbiHeaderField) -> AbiFaultCandidate {
    let descriptor = DmaAddress::new(0x1000);
    let fault = match field {
        AbiHeaderField::Version => AbiFault::invalid_version(descriptor, 9),
        AbiHeaderField::Size => AbiFault::invalid_size(descriptor, 63),
        AbiHeaderField::Opcode => AbiFault::invalid_opcode(descriptor, 9),
        AbiHeaderField::Flags => AbiFault::invalid_flags(descriptor, 1),
    };
    candidate(fault, AbiFaultOrder::Header(field))
}

fn reserved(field: AbiReservedField, offset: u64) -> AbiFaultCandidate {
    candidate(
        AbiFault::invalid_reserved(DmaAddress::new(0x1000), offset, 7),
        AbiFaultOrder::Reserved(field),
    )
}

fn scalar(field: AbiScalarField, offset: u64) -> AbiFaultCandidate {
    candidate(
        AbiFault::value_out_of_range(
            DmaAddress::new(0x1000),
            offset,
            u64::from(u32::MAX) + 1,
        ),
        AbiFaultOrder::Scalar(field),
    )
}

fn candidate(fault: AbiFault, order: AbiFaultOrder) -> AbiFaultCandidate {
    match AbiFaultCandidate::try_new(fault, order) {
        Ok(candidate) => candidate,
        Err(error) => {
            eprintln!("fixture fault must match typed order: {error}");
            std::process::abort();
        }
    }
}
