use pdma_model_contract::abi::{
    ABI_VERSION, AbiCommand, DESCRIPTOR_SIZE, DescriptorDecodeError, DescriptorHeaderSchema,
    DescriptorHeaderSchemaOpcode, DescriptorSchema, DescriptorSchemaPayloadBroadcastCopyValues,
    DescriptorSchemaPayloadCopyValues, DescriptorSchemaViewMut, DescriptorViolationKind,
};

const COPY: [u8; 64] = [
    1, 0, 1, 0, 0, 0, 64, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 32,
    0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn set_u16(bytes: &mut [u8; 64], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u64(bytes: &mut [u8; 64], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn has_violation(bytes: &[u8], expected: DescriptorViolationKind) -> bool {
    matches!(
        AbiCommand::decode(bytes),
        Err(DescriptorDecodeError::Violation(violation)) if violation.kind() == expected
    )
}

fn main() {
    let mut version = COPY;
    set_u16(&mut version, 0, ABI_VERSION + 1);
    set_u16(&mut version, 6, DESCRIPTOR_SIZE - 1);
    set_u16(&mut version, 2, 9);
    let mut opcode = COPY;
    set_u16(&mut opcode, 2, 9);
    set_u16(&mut opcode, 4, 1);
    set_u64(&mut opcode, 40, 1);
    let mut flags = COPY;
    set_u16(&mut flags, 4, 1);
    set_u64(&mut flags, 40, 1);
    let mut reserved = COPY;
    set_u64(&mut reserved, 40, 1);
    set_u64(&mut reserved, 32, u64::from(u32::MAX) + 1);
    let mut scalar = COPY;
    set_u64(&mut scalar, 32, u64::from(u32::MAX) + 1);
    let mut size = COPY;
    set_u16(&mut size, 6, DESCRIPTOR_SIZE - 1);
    let mut count = COPY;
    set_u16(&mut count, 2, 3);
    set_u64(&mut count, 40, 0);
    let mut alignment = COPY;
    set_u16(&mut alignment, 2, 3);
    set_u64(&mut alignment, 24, 0x2003);
    set_u64(&mut alignment, 40, 1);
    let version_kind = DescriptorViolationKind::Header(
        DescriptorHeaderSchema::VERSION_IDENTITY.at_byte(DescriptorHeaderSchema::VERSION_OFFSET),
    );
    let size_kind = DescriptorViolationKind::Header(
        DescriptorHeaderSchema::SIZE_IDENTITY.at_byte(DescriptorHeaderSchema::SIZE_OFFSET),
    );
    let opcode_kind = DescriptorViolationKind::Header(
        DescriptorHeaderSchema::OPCODE_IDENTITY.at_byte(DescriptorHeaderSchema::OPCODE_OFFSET),
    );
    let flags_kind = DescriptorViolationKind::Header(
        DescriptorHeaderSchema::FLAGS_IDENTITY.at_byte(DescriptorHeaderSchema::FLAGS_OFFSET),
    );
    let reserved_kind = DescriptorViolationKind::Reserved {
        opcode: DescriptorHeaderSchemaOpcode::Copy,
        location: DescriptorSchema::PAYLOAD_IDENTITY.at_byte(DescriptorSchema::WORD3_OFFSET),
    };
    let scalar_kind = DescriptorViolationKind::Scalar(
        DescriptorSchemaPayloadCopyValues::LENGTH_IDENTITY
            .at_byte(DescriptorSchema::WORD2_OFFSET),
    );
    let count_kind = DescriptorViolationKind::Scalar(
        DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_COUNT_IDENTITY
            .at_byte(DescriptorSchema::WORD3_OFFSET),
    );
    let alignment_kind = DescriptorViolationKind::Scalar(
        DescriptorSchemaPayloadBroadcastCopyValues::DESTINATION_LIST_DMA_ADDRESS_IDENTITY
            .at_byte(DescriptorSchema::WORD1_OFFSET),
    );
    let observed = [
        matches!(AbiCommand::decode(&COPY[..63]), Err(DescriptorDecodeError::Codec(_))),
        has_violation(&version, version_kind),
        has_violation(&size, size_kind),
        has_violation(&opcode, opcode_kind),
        has_violation(&flags, flags_kind),
        has_violation(&reserved, reserved_kind),
        has_violation(&scalar, scalar_kind),
        has_violation(&count, count_kind),
        has_violation(&alignment, alignment_kind),
    ];
    assert!(observed.into_iter().all(|value| value));
    let mut destination = [0x5a_u8; 63];
    let before = destination;
    let write = DescriptorSchemaViewMut::new(&mut destination).and_then(|mut view| {
        let mut header = view.header_mut()?;
        header.set_version(ABI_VERSION)
    });
    assert!(write.is_err());
    println!(
        "short;violations=version,size,opcode,flags,reserved,scalar,count,alignment;unchanged={}",
        destination == before
    );
}
