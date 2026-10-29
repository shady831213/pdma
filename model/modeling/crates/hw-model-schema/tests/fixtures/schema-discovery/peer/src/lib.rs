//! A second project contributes metadata without an explicit catalog declaration.
pub mod messages {
    use schema::record_schema;
    record_schema! { pub record Entry { alignment: 4, endian: Little, fields: { value: u32 } } }
    // No caller refers to this Rust type; discovery must survive release/LTO.
    record_schema! { pub record Dormant { alignment: 4, endian: Little, fields: { value: u32 } } }
}
