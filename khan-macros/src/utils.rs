use crate::prelude::*;
use proc_macro_crate::{FoundCrate, crate_name};

macro_rules! extract {
    ($val:expr, $pat:pat, $error_message: expr) => {
        let $pat = $val else {
            return Err(Error::new_spanned($val, $error_message));
        };
    };
}

pub(crate) use extract;

pub fn extract_named_fields(span: Span, data: Data) -> Result<FieldsNamed> {
    let Data::Struct(data_struct) = data else {
        return Err(Error::new(span, "expected struct"));
    };

    extract!(
        data_struct.fields,
        Fields::Named(named_fields),
        "expected named fields"
    );

    Ok(named_fields)
}

pub fn extract_serde_rename(field: &Field) -> Option<String> {
    #[derive(FromAttributes)]
    #[darling(attributes(serde))]
    struct SerdeAttribute {
        rename: String,
    }

    let serde_attribute = SerdeAttribute::from_attributes(&field.attrs).ok();

    serde_attribute.map(|attribute| attribute.rename)
}

pub fn build_fields_enum<'a>(
    field_idents: impl Iterator<Item = &'a Ident>,
    field_lits: impl Iterator<Item = &'a LitStr>,
) -> TokenStream {
    let field_idents_upper_camel_case = field_idents
        .map(|ident| Ident::new(&ident.to_string().to_upper_camel_case(), Span::call_site()))
        .collect_vec();

    quote! {
        #[derive(::std::fmt::Debug)]
        pub enum Fields {
            #( #field_idents_upper_camel_case ),*
        }

        impl ::std::fmt::Display for Fields {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::write!(
                    f,
                    "{}",
                    match self {
                        #(
                            #field_idents_upper_camel_case => #field_lits
                        ),*
                    }
                )
            }
        }

        impl ::std::convert::From<Fields> for ::std::string::String {
            fn from(value: Fields) -> Self {
                ::std::string::ToString::to_string(&value)
            }
        }
    }
}

pub fn krate() -> TokenStream {
    match crate_name("khan").unwrap() {
        FoundCrate::Itself => quote! { crate },
        FoundCrate::Name(name) => quote! { #name },
    }
}

pub fn mongodb() -> TokenStream {
    match crate_name("khan").unwrap() {
        FoundCrate::Itself => quote! { ::mongodb },
        FoundCrate::Name(name) => quote! { #name::mongodb },
    }
}
