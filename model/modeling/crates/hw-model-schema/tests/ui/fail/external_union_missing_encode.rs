use hw_model_schema::{SchemaCodec, SchemaCodecError, record_schema, union_schema};
record_schema! { record Item { endian: Little, fields: { value: u32 } } }
union_schema! { union Packet { cases: { Item(Item) }, codec: external } }
impl SchemaCodec for Packet {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaCodecError> {
        Self::decode_variant(bytes, PacketCase::Item)
    }
}
fn main() {}
