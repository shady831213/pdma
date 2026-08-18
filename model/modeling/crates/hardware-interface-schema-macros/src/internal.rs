#[path = "crate_path.rs"]
mod crate_path;
#[path = "expand/mod.rs"]
mod expand;
#[path = "syntax/mod.rs"]
mod syntax;
#[path = "validate/mod.rs"]
mod validate;

impl crate::Expansion for () {
    fn field(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::field::run(input)
    }

    fn record(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::record::run(input)
    }

    fn register(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::register::run(input)
    }

    fn register_field(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::register_field::run(input)
    }

    fn map(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::map::run(input)
    }
}
