use hardware_interface_schema::record_schema;

record_schema! {
    pub record Bad {
        endian: Little,
        fields: { code: enum(u64) { variants: { Last = 18446744073709551615, Overflow } } },
    }
}

fn main() {}
