use hw_model_schema::{SchemaCodec, SchemaCodecError, SchemaLayout, record_schema};
record_schema! { record Wide { endian: Little, fields: { value: u64 } } }
struct Bad;
impl SchemaLayout for Bad {
    type Value = Wide;
    type View<'a> = WideView<'a>;
    type ViewMut<'a> = WideViewMut<'a>;
    const NAME: &'static str = "Bad";
    const ALIGNMENT: usize = 3;
    const EXTENT: usize = 8;
    const ZERO: Wide = Wide::ZERO;
    fn view(bytes: &[u8]) -> Result<Self::View<'_>, SchemaCodecError> { WideView::new(bytes) }
    fn view_mut(bytes: &mut [u8]) -> Result<Self::ViewMut<'_>, SchemaCodecError> { WideViewMut::new(bytes) }
}
impl SchemaCodec for Bad {
    fn decode(bytes: &[u8]) -> Result<Wide, SchemaCodecError> { Wide::from_bytes(bytes) }
    fn encode(bytes: &mut [u8], value: &Wide) -> Result<(), SchemaCodecError> { value.encode_into(bytes) }
}
record_schema! { record Parent { endian: Little, fields: {
    tag: enum(u8) { variants: { Bad = 0, Wide = 1 } },
    payload: tagged_union { tag: self.tag, cases: { Bad(Bad), Wide(Wide) } },
} } }
fn main() {}
