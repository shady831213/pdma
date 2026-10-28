use hw_model_schema::{RegisterBank, StaticRegisterLayout, register_schema};
register_schema! { pub register Word {
    width: 4, endian: Little, access: ReadWrite,
    reset: Constant(0), writable_mask: 0xffff_ffff, fields: {},
} }
fn main() {
    let _ = RegisterBank::new(StaticRegisterLayout::<Word>::new(), 0, 2, 16);
}
