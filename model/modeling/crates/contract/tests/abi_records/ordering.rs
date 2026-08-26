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
                header_location(
                    DescriptorHeaderSchema::FLAGS_IDENTITY,
                    DescriptorHeaderSchema::FLAGS_OFFSET,
                ),
                header_location(
                    DescriptorHeaderSchema::OPCODE_IDENTITY,
                    DescriptorHeaderSchema::OPCODE_OFFSET,
                ),
                header_location(
                    DescriptorHeaderSchema::SIZE_IDENTITY,
                    DescriptorHeaderSchema::SIZE_OFFSET,
                ),
                header_location(
                    DescriptorHeaderSchema::VERSION_IDENTITY,
                    DescriptorHeaderSchema::VERSION_OFFSET,
                ),
            ],
            (AbiEventKind::InvalidVersion, 0x1000, 9, 1, 1, 0, 0),
        ),
        header_case(
            "size before opcode flags",
            &[
                header_location(
                    DescriptorHeaderSchema::FLAGS_IDENTITY,
                    DescriptorHeaderSchema::FLAGS_OFFSET,
                ),
                header_location(
                    DescriptorHeaderSchema::OPCODE_IDENTITY,
                    DescriptorHeaderSchema::OPCODE_OFFSET,
                ),
                header_location(
                    DescriptorHeaderSchema::SIZE_IDENTITY,
                    DescriptorHeaderSchema::SIZE_OFFSET,
                ),
            ],
            (AbiEventKind::InvalidSize, 0x1000, 63, 64, 1, 0, 6),
        ),
        header_case(
            "opcode before flags",
            &[
                header_location(
                    DescriptorHeaderSchema::FLAGS_IDENTITY,
                    DescriptorHeaderSchema::FLAGS_OFFSET,
                ),
                header_location(
                    DescriptorHeaderSchema::OPCODE_IDENTITY,
                    DescriptorHeaderSchema::OPCODE_OFFSET,
                ),
            ],
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
            scalar(
                DescriptorSchemaPayloadCopyValues::LENGTH_IDENTITY
                    .at_byte(DescriptorSchema::WORD2_OFFSET),
            ),
            reserved(DescriptorHeaderSchemaOpcode::Copy, DescriptorSchema::WORD3_OFFSET),
            header(header_location(
                DescriptorHeaderSchema::FLAGS_IDENTITY,
                DescriptorHeaderSchema::FLAGS_OFFSET,
            )),
        ],
        (AbiEventKind::InvalidFlags, 0x1000, 1, 0, 1, 0, 4),
    )
}

fn header_case(
    name: &'static str,
    fields: &[SchemaLocation],
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
            reserved(DescriptorHeaderSchemaOpcode::Copy, DescriptorSchema::WORD5_OFFSET),
            reserved(DescriptorHeaderSchemaOpcode::Copy, DescriptorSchema::WORD4_OFFSET),
            reserved(DescriptorHeaderSchemaOpcode::Copy, DescriptorSchema::WORD3_OFFSET),
        ],
        (AbiEventKind::InvalidReserved, 0x1000, 7, 0, 1, 0, 40),
    )
}

fn scalar_case() -> ConflictCase {
    case(
        "scalar ascending payload offset",
        vec![
            scalar(
                DescriptorSchemaPayloadStridedCopyValues::DESTINATION_STRIDE_IDENTITY
                    .at_byte(DescriptorSchema::WORD5_OFFSET),
            ),
            scalar(
                DescriptorSchemaPayloadStridedCopyValues::SOURCE_STRIDE_IDENTITY
                    .at_byte(DescriptorSchema::WORD4_OFFSET),
            ),
            scalar(
                DescriptorSchemaPayloadStridedCopyValues::ROW_BYTES_IDENTITY
                    .at_byte(DescriptorSchema::WORD3_OFFSET),
            ),
            scalar(
                DescriptorSchemaPayloadStridedCopyValues::ROWS_IDENTITY
                    .at_byte(DescriptorSchema::WORD2_OFFSET),
            ),
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

const fn header_location(identity: SchemaMemberIdentity, offset: usize) -> SchemaLocation {
    identity.at_byte(offset)
}

fn header(location: SchemaLocation) -> AbiFaultCandidate {
    let descriptor = DmaAddress::new(0x1000);
    let fault = if location
        == header_location(
            DescriptorHeaderSchema::VERSION_IDENTITY,
            DescriptorHeaderSchema::VERSION_OFFSET,
        )
    {
        AbiFault::invalid_version(descriptor, 9)
    } else if location
        == header_location(
            DescriptorHeaderSchema::SIZE_IDENTITY,
            DescriptorHeaderSchema::SIZE_OFFSET,
        )
    {
        AbiFault::invalid_size(descriptor, 63)
    } else if location
        == header_location(
            DescriptorHeaderSchema::OPCODE_IDENTITY,
            DescriptorHeaderSchema::OPCODE_OFFSET,
        )
    {
        AbiFault::invalid_opcode(descriptor, 9)
    } else if location
        == header_location(
            DescriptorHeaderSchema::FLAGS_IDENTITY,
            DescriptorHeaderSchema::FLAGS_OFFSET,
        )
    {
        AbiFault::invalid_flags(descriptor, 1)
    } else {
        std::process::abort()
    };
    candidate(
        fault,
        AbiFaultOrder::Descriptor(DescriptorViolationKind::Header(location)),
    )
}

fn reserved(opcode: DescriptorHeaderSchemaOpcode, offset: usize) -> AbiFaultCandidate {
    let location = DescriptorSchema::PAYLOAD_IDENTITY.at_byte(offset);
    candidate(
        AbiFault::invalid_reserved(DmaAddress::new(0x1000), location_index(location), 7),
        AbiFaultOrder::Descriptor(DescriptorViolationKind::Reserved { opcode, location }),
    )
}

fn scalar(location: SchemaLocation) -> AbiFaultCandidate {
    candidate(
        AbiFault::value_out_of_range(
            DmaAddress::new(0x1000),
            location_index(location),
            u64::from(u32::MAX) + 1,
        ),
        AbiFaultOrder::Descriptor(DescriptorViolationKind::Scalar(location)),
    )
}

fn location_index(location: SchemaLocation) -> u64 {
    let SchemaCoordinate::Byte(offset) = location.coordinate() else {
        std::process::abort()
    };
    u64::try_from(offset).unwrap_or_else(|_| std::process::abort())
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
