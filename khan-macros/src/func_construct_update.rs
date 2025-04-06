use crate::prelude::*;

struct Input {
    module: Ident,
    fields: Punctuated<Field, Token![,]>,
}

impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let constructor = input.parse()?;
        input.parse::<Token![,]>()?;
        let fields = Punctuated::parse_terminated(input)?;
        Ok(Self {
            module: constructor,
            fields,
        })
    }
}

struct Field {
    ident: Ident,
    value: Expr,
}

impl Parse for Field {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let value = input.parse()?;

        Ok(Self { ident, value })
    }
}

pub fn func_construct_update(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<Input>(input)?;

    let output = build(&input);

    Ok(output)
}

fn build(input: &Input) -> TokenStream {
    let krate = krate();
    let module = &input.module;

    let fields = input.fields.iter().map(|field| {
        let ident = &field.ident;
        let value = &field.value;

        quote! {
            #ident: #krate::Field::Set(#value)
        }
    });

    quote! {
        #module::TypedUpdate {
            #( #fields, )*
            ..std::default::Default::default()
        }
    }
}
