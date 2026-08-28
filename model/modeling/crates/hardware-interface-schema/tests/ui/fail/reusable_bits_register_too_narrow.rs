use hardware_interface_schema::{bits_schema, register_schema};

bits_schema! {
    pub bits WideBits(u16) {
        high: 8..9,
    }
}

register_schema! {
    pub register NarrowRegister {
        width: 1,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: WideBits,
    }
}

fn main() {}
