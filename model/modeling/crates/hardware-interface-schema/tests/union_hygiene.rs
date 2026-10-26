//! Union field names must not capture encoder bookkeeping variables.

use hardware_interface_schema::{bits_schema, record_schema};

bits_schema! { pub bits Byte(u64) { value: 0..=7 } }

record_schema! {
    pub record Packet {
        alignment: 8, endian: Little,
        fields: {
            kind: enum(u8) { variants: { Scalars = 0, Bits = 1 } },
            payload: tagged_union {
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

mod caller_names {
    use hardware_interface_schema::{bits_schema, record_schema, register_map, register_schema};

    type Result<T> = core::result::Result<T, ()>;
    type Option<T> = core::result::Result<T, ()>;
    struct Default;
    struct Copy;
    struct Vec;
    enum Reply {
        Ok,
        Err,
        Some,
        None,
    }
    use Reply::{Err, None, Ok, Some};

    bits_schema! { pub bits Flags(u8) { enabled: 0..=0 } }
    register_schema! {
        pub register Word {
            width: 4, endian: Little, access: ReadWrite,
            reset: Zero(0), writable_mask: 0xffff_ffff, fields: {},
        }
    }
    register_map! {
        pub map Block { alignment: 4, placements: [pub value: Word] }
    }
    register_map! {
        pub map Bank(count: usize) { alignment: 4, placements: [pub blocks: Block[count]] }
    }

    record_schema! { pub record Item { endian: Big, fields: { value: u16 } } }
    record_schema! {
        pub record Message {
            endian: Little,
            fields: {
                tag: enum(u8) { variants: { Item = 0 } },
                payload: tagged_union { tag: self.tag, cases: { Item(Item) } },
            },
        }
    }

    #[test]
    fn record_union_ignores_caller_prelude_names() -> Result<()> {
        // Given: application names shadow standard traits, types and constructors.
        std::hint::black_box((Default, Copy, Vec, Ok, Err, Some, None));
        let application_option: Option<()> = core::result::Result::Ok(());
        assert!(application_option.is_ok());
        let message = Message::new(
            MessageTag::Item,
            MessagePayloadValues::Item(Item { value: 0x1234 }),
        );
        // When: an ordinary caller uses the generated codec and Default implementation.
        let mut bytes = message.to_bytes().map_err(|_| ())?;
        let decoded = Message::from_bytes(&bytes).map_err(|_| ())?;
        let initial = <MessagePayloadValues as core::default::Default>::default();
        // Then: nested endian and failure behavior do not depend on caller imports.
        assert_eq!(decoded, message);
        assert_eq!(&bytes[2..4], &[0x12, 0x34]);
        assert_eq!(initial, MessagePayloadValues::Item(Item { value: 0 }));
        bytes[0] = 1;
        assert!(matches!(
            Message::from_bytes(&bytes),
            core::result::Result::Err(
                hardware_interface_schema::SchemaCodecError::UnknownTag { .. }
            )
        ));
        core::result::Result::Ok(())
    }

    #[test]
    fn register_and_bits_macros_ignore_caller_prelude_names() -> Result<()> {
        // Given: the same caller scope also declares bits and a register window.
        let flags = FlagsValues { enabled: true };
        let mut window = hardware_interface_schema::RegisterWindow::<Block>::new(16);
        // When: the generated register map receives real byte writes and dispatched reads.
        window.write(16, &[7, 0, 0, 0]).map_err(|_| ())?;
        let mut bytes = [0; 4];
        hardware_interface_schema::register_dispatch!(
            window.read(0, &mut bytes),
            window.read(16, &mut bytes)
        )
        .map_err(|_| ())?;
        // Then: all public macro surfaces preserve values without relying on ambient names.
        assert_eq!(bytes, [7, 0, 0, 0]);
        assert_eq!(flags.raw().map_err(|_| ())?, 1);
        assert_eq!(
            Bank::new(2).map_err(|_| ())?.blocks().offset(1),
            core::option::Option::Some(4)
        );
        core::result::Result::Ok(())
    }
}
