mod crate_path;
mod expand;
mod syntax;
mod validate;

impl crate::Expansion for () {
    fn bits(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::bits::run(input)
    }

    fn record(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::record::run(input)
    }

    fn register(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::register::run(input)
    }

    fn map(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        expand::map::run(input)
    }
}
