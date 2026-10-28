use hw_model_schema::{SchemaCodec, SchemaCodecError, record_schema, union_schema};
record_schema! { record Item { endian: Little, fields: { value: u32 } } }
union_schema! { union Packet { cases: { Item(Item) }, codec: external } }
impl SchemaCodec for Packet {
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        value.encode_variant(bytes)
    }
}
fn main() {}
