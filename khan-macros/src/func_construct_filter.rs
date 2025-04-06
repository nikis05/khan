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
    operator: Option<Ident>,
    value: Expr,
}

impl Parse for Field {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let ident = input.parse()?;
        input.parse::<Token![:]>()?;

        let operator_or_value = input.parse::<Expr>()?;

        let mut operator_and_operand = None;

        if let Expr::Call(expr_call) = &operator_or_value {
            if let Expr::Path(expr_path) = expr_call.func.as_ref() {
                if let Some(ident) = expr_path.path.get_ident() {
                    if (ident == "Eq"
                        || ident == "Ne"
                        || ident == "Gt"
                        || ident == "Gte"
                        || ident == "Lt"
                        || ident == "Lte"
                        || ident == "In"
                        || ident == "Nin")
                        && expr_call.args.len() == 1
                    {
                        operator_and_operand = Some((ident, expr_call.args[0].clone()));
                    }
                }
            }
        }

        let output = match operator_and_operand {
            Some((operator, operand)) => Self {
                ident,
                operator: Some(operator.to_owned()),
                value: operand,
            },
            None => Self {
                ident,
                operator: None,
                value: operator_or_value,
            },
        };

        Ok(output)
    }
}

pub fn func_construct_filter(input: TokenStream) -> Result<TokenStream> {
    let input = parse2::<Input>(input)?;

    let output = build(&input);

    Ok(output)
}

fn build(input: &Input) -> TokenStream {
    let krate = krate();
    let module = &input.module;

    let fields = input.fields.iter().map(|field| {
        let ident = &field.ident;
        let operator = field
            .operator
            .clone()
            .unwrap_or_else(|| parse_quote! { Eq });
        let value = &field.value;

        quote! {
            #ident: #krate::Field::Set(#krate::FilterOperator::#operator(#value))
        }
    });

    quote! {
        #module::TypedFilter {
            #( #fields, )*
            ..std::default::Default::default()
        }
    }
}
