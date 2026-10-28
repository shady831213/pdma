use hw_model_schema::register_schema;

register_schema! {
    #[short_description = "Read/write word"]
    #[description = "A 32-bit little-endian register, initially zero. Software may read or write all bits; its placement defines their hardware meaning."]
    pub register Word {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xffff_ffff,
        fields: {},
    }
}

register_schema! {
    #[short_description = "Read-only word"]
    #[description = "A 32-bit little-endian register, initially zero. Hardware updates the value; software may read it but cannot write it."]
    pub register ReadWord {
        width: 4,
        endian: Little,
        access: ReadOnly,
        reset: Zero(0),
        writable_mask: 0,
        fields: {},
    }
}

register_schema! {
    #[short_description = "Memory address"]
    #[description = "A 64-bit little-endian address in the device's attached memory backend. The backend determines address interpretation; the value need not be a host pointer."]
    pub register Address {
        width: 8,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 0xffff_ffff_ffff_ffff,
        fields: {},
    }
}

register_schema! {
    #[short_description = "Enable control"]
    #[description = "A 32-bit little-endian control register, initially disabled. Only bit zero is writable; bits 1 through 31 are reserved and remain zero."]
    pub register Switch {
        width: 4,
        endian: Little,
        access: ReadWrite,
        reset: Zero(0),
        writable_mask: 1,
        fields: {
            #[short_description = "Enabled"]
            #[description = "Zero disables the function and one enables it. The containing register placement specifies the controlled function and its side effects."]
            enabled: 0..=0,
        },
    }
}
