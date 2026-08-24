#![allow(clippy::redundant_pub_crate)]

pub(crate) use hardware_interface_schema::{Endian, RecordSpec, ScalarKind, SchemaCodecError};

pub(crate) const COPY: [u8; 64] = [
    1, 0, 1, 0, 0, 0, 64, 0, 0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 1, 0x80, 0x70, 0x60, 0x50,
    0x40, 0x30, 0x20, 0x10, 0, 0xf0, 0xe0, 0xd0, 0xc0, 0xb0, 0xa0, 0x90, 0x44, 0x33, 0x22, 0x11, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
pub(crate) const COMPLETION_BYTES: [u8; 32] = [
    1, 0, 32, 0, 0, 0, 0, 0, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 0x44, 0x33, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0,
];
pub(crate) const EVENT_BYTES: [u8; 64] = [
    1, 0, 64, 0, 1, 0, 0, 0, 0x22, 0x11, 0, 0, 0, 0, 0, 0, 0x44, 0x33, 0, 0, 0, 0, 0, 0, 0, 0x20,
    0, 0, 0, 0, 0, 0, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 1, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];
pub(crate) const INDIRECT_BYTES: [u8; 8] = [0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 1];

type Field = (&'static str, u16, u16, ScalarKind);

pub(crate) const DESCRIPTOR_FIELDS: [Field; 11] = [
    ("version", 0, 2, ScalarKind::U16),
    ("opcode", 2, 2, ScalarKind::U16),
    ("flags", 4, 2, ScalarKind::U16),
    ("size", 6, 2, ScalarKind::U16),
    ("command_id", 8, 8, ScalarKind::U64),
    ("word0", 16, 8, ScalarKind::U64),
    ("word1", 24, 8, ScalarKind::U64),
    ("word2", 32, 8, ScalarKind::U64),
    ("word3", 40, 8, ScalarKind::U64),
    ("word4", 48, 8, ScalarKind::U64),
    ("word5", 56, 8, ScalarKind::U64),
];
pub(crate) const COMPLETION_FIELDS: [Field; 7] = [
    ("version", 0, 2, ScalarKind::U16),
    ("size", 2, 2, ScalarKind::U16),
    ("status", 4, 2, ScalarKind::U16),
    ("flags", 6, 2, ScalarKind::U16),
    ("origin_queue_id", 8, 8, ScalarKind::U64),
    ("command_id", 16, 8, ScalarKind::U64),
    ("reserved", 24, 8, ScalarKind::U64),
];
pub(crate) const EVENT_FIELDS: [Field; 13] = [
    ("version", 0, 2, ScalarKind::U16),
    ("size", 2, 2, ScalarKind::U16),
    ("kind", 4, 2, ScalarKind::U16),
    ("flags", 6, 2, ScalarKind::U16),
    ("origin_queue_id", 8, 8, ScalarKind::U64),
    ("command_id", 16, 8, ScalarKind::U64),
    ("fault_locator", 24, 8, ScalarKind::U64),
    ("detail0", 32, 8, ScalarKind::U64),
    ("detail1", 40, 8, ScalarKind::U64),
    ("stage", 48, 2, ScalarKind::U16),
    ("access", 50, 2, ScalarKind::U16),
    ("reserved", 52, 4, ScalarKind::U32),
    ("index", 56, 8, ScalarKind::U64),
];
pub(crate) const INDIRECT_FIELDS: [Field; 1] = [("destination_dma_address", 0, 8, ScalarKind::U64)];

pub(crate) fn assert_schema_parity(schema: RecordSpec, expected: &[Field]) {
    assert_eq!(schema.fields().len(), expected.len());
    for (field, &(name, offset, width, scalar)) in schema.fields().iter().zip(expected) {
        assert_eq!(
            (
                field.name(),
                field.offset(),
                field.width(),
                field.scalar(),
                field.endian()
            ),
            (
                name,
                usize::from(offset),
                usize::from(width),
                scalar,
                Endian::Little
            )
        );
    }
}
