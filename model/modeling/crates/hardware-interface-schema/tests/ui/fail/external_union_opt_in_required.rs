use hardware_interface_schema::{Encode, record_schema, union_schema};
record_schema! { record Item { endian: Little, fields: { value: u32 } } }
union_schema! { union Packet { cases: { Item(Item) } } }
fn main() {
    let _ = Packet::ZERO.encode();
    let _ = Packet::ZERO.to_bytes();
    let _ = PacketView::new(&[0; 4]).unwrap().values();
}
