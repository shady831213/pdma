use super::vectors;
use pdma_model_contract::abi::{
    DescriptorHeaderSchemaFlagsValues, DescriptorHeaderSchemaOpcode, DescriptorHeaderSchemaValues,
    DescriptorSchemaPayloadCopyValues, DescriptorSchemaPayloadValues, DescriptorSchemaValues,
};

#[test]
fn generated_descriptor_uses_semantic_header_and_payload_fields() {
    let values = DescriptorSchemaValues::from_bytes(&vectors::COPY);
    assert!(values.is_ok());
    let Ok(values) = values else {
        return;
    };
    assert_eq!(
        values,
        DescriptorSchemaValues {
            header: DescriptorHeaderSchemaValues {
                version: 1,
                opcode: DescriptorHeaderSchemaOpcode::Copy,
                flags: DescriptorHeaderSchemaFlagsValues::ZERO,
                command_id: 0x0123_4567_89ab_cdef,
            },
            payload: DescriptorSchemaPayloadValues::Copy(DescriptorSchemaPayloadCopyValues {
                source_dma_address: 0x1020_3040_5060_7080,
                destination_dma_address: 0x90a0_b0c0_d0e0_f000,
                length: 0x1122_3344,
            }),
        }
    );
    assert_eq!(values.to_bytes(), Ok(vectors::COPY));
}
