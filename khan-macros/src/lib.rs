#[warn(clippy::pedantic)]
#[allow(clippy::too_many_lines)]
mod derive_entity;
mod derive_fields;
mod func_construct_filter;
mod func_construct_update;
mod prelude;
mod utils;

fn expand<F: FnOnce(proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream>>(
    fun: F,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    fun(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Entity, attributes(entity))]
pub fn entity(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand(derive_entity::derive_entity, input)
}

#[proc_macro_derive(Fields)]
pub fn fields(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand(derive_fields::derive_fields, input)
}

#[proc_macro]
pub fn construct_filter(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand(func_construct_filter::func_construct_filter, input)
}

#[proc_macro]
pub fn construct_update(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand(func_construct_update::func_construct_update, input)
}
