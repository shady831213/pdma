use hardware_interface_schema::bits_schema;

bits_schema! {
    pub bits Bad(u16) {
        code: 7..10 => enum { variants: { Good = 0, Bad = 8 } },
    }
}

fn main() {}
