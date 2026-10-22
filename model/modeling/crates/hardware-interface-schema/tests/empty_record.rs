#![doc = "Empty record raw storage is a transparent zero-size byte owner."]
use hardware_interface_schema::{Decode, Encode, record_schema};

record_schema! {
    record Empty { alignment: 8, endian: Little, fields: {} }
}

#[test]
fn empty_record_roundtrips_without_storage() {
    let record = Empty {};
    let raw = record.encode().unwrap();
    assert_eq!(raw.as_bytes(), &[]);
    assert_eq!(raw.decode().unwrap(), record);
    assert_eq!(std::mem::size_of::<EmptyRaw>(), 0);
}
