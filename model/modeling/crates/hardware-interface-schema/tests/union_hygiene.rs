//! Union field names must not capture encoder bookkeeping variables.

use hardware_interface_schema::{bits_schema, record_schema};

bits_schema! { pub bits Byte(u64) { value: 0..=7 } }

record_schema! {
    pub record Packet {
        alignment: 8, endian: Little,
        fields: {
            kind: enum(u8) { variants: { Scalars = 0, Bits = 1 } },
            payload: union {
                tag: self.kind,
                cases: {
                    Scalars { offset: u64, base: u64, bytes: u64, end: u64, actual: u64 },
                    Bits { offset: Byte, base: Byte, bytes: Byte, end: Byte, actual: Byte },
                },
            },
        },
    }
}

#[test]
fn union_field_names_round_trip_without_capturing_encoder_locals()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: both scalar and named-bitfield cases use the encoder's own local names.
    let values = [
        PacketPayloadValues::Scalars(PacketPayloadScalarsValues {
            offset: 11,
            base: 22,
            bytes: 33,
            end: 44,
            actual: 55,
        }),
        PacketPayloadValues::Bits(PacketPayloadBitsValues {
            offset: ByteValues { value: 11 },
            base: ByteValues { value: 22 },
            bytes: ByteValues { value: 33 },
            end: ByteValues { value: 44 },
            actual: ByteValues { value: 55 },
        }),
    ];
    for payload in values {
        let packet = Packet {
            kind: payload.tag(),
            payload,
        };
        // When: the generated codec serializes and decodes the actual public value.
        let bytes = packet.to_bytes()?;
        let decoded = Packet::from_bytes(&bytes)?;
        // Then: all fields retain their values, including independent known wire slots.
        assert_eq!(decoded, packet);
        assert_eq!(
            [bytes[8], bytes[16], bytes[24], bytes[32], bytes[40]],
            [11, 22, 33, 44, 55]
        );
    }
    Ok(())
}
