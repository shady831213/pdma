use hardware_interface_schema::bits_schema;

bits_schema! {
    pub bits Bad(u8) {
        code: 0..2 => enum { variants: { Same = 0, Same = 1 } },
    }
}

fn main() {}
