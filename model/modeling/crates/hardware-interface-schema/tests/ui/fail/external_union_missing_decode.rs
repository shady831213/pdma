use hardware_interface_schema::{SchemaCodecError, record_schema, union_schema};
record_schema! { record Item { endian: Little, fields: { value: u32 } } }
union_schema! { union Packet { cases: { Item(Item) }, codec: external } }
impl Packet {
    fn encode_into(&self, bytes: &mut [u8]) -> Result<(), SchemaCodecError> {
        self.encode_variant(bytes)
    }
}
fn main() {}
