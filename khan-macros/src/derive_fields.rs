use crate::{
    prelude::*,
    utils::{build_fields_enum, extract_named_fields, extract_serde_rename},
};

pub fn derive_fields(item: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(item)?;

    let fields_named = extract_named_fields(input.span(), input.data)?;

    let fields = fields_named
        .named
        .into_iter()
        .map(|field| {
            let rename = extract_serde_rename(&field);
            (field.ident.unwrap(), rename)
        })
        .collect_vec();

    let output = build(&input.vis, &input.ident, &fields);

    Ok(output)
}

fn build(vis: &Visibility, ident: &Ident, fields: &[(Ident, Option<String>)]) -> TokenStream {
    let mod_ident = Ident::new(&ident.to_string().to_snake_case(), Span::call_site());

    let field_idents = fields.iter().map(|field| &field.0);
    let field_lits = fields
        .iter()
        .map(|field| {
            LitStr::new(
                &field
                    .1
                    .as_deref()
                    .map(Cow::Borrowed)
                    .unwrap_or_else(|| Cow::Owned(field.0.to_string())),
                Span::call_site(),
            )
        })
        .collect_vec();

    let fields_enum = build_fields_enum(field_idents, field_lits.iter());

    quote! {
        #vis mod #mod_ident {
            #fields_enum
        }
    }
}
