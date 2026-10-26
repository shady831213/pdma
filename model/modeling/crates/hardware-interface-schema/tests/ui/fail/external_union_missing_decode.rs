use hardware_interface_schema::{SchemaCodec, SchemaCodecError, record_schema, union_schema};
record_schema! { record Item { endian: Little, fields: { value: u32 } } }
union_schema! { union Packet { cases: { Item(Item) }, codec: external } }
impl SchemaCodec for Packet {
    type Value = Self;
    type View<'a> = PacketView<'a>;
    type ViewMut<'a> = PacketViewMut<'a>;
    const NAME: &'static str = "Packet";
    const ALIGNMENT: usize = Self::RECORD_ALIGNMENT;
    const EXTENT: usize = Self::RECORD_SIZE;
    const ZERO: Self = Self::ZERO;
    fn encode(bytes: &mut [u8], value: &Self) -> Result<(), SchemaCodecError> {
        value.encode_variant(bytes)
    }
    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError> {
        PacketView::new(bytes)
    }
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError> {
        PacketViewMut::new(bytes)
    }
}
fn main() {}
