use hw_model_schema::record_schema;
record_schema! { pub record Conflict {
    endian: Little, fields: {
        kind: enum(u8) { variants: { Data = 0 } },
        payload: tagged_union { tag: self.kind, cases: { Data { data: u8 } } },
        WORD0_OFFSET: u8,
    },
} }
fn main() {}
