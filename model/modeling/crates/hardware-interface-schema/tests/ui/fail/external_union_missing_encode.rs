use hardware_interface_schema::{SchemaCodecError, record_schema, union_schema};
record_schema! { record Item { endian: Little, fields: { value: u32 } } }
union_schema! { union Packet { cases: { Item(Item) }, codec: external } }
impl Packet {
    fn from_bytes(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        Self::decode_variant(bytes, PacketCase::Item)
    }
}
fn main() {}
