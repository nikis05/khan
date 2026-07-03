use syn::ItemFn;

use crate::prelude::*;

pub fn attr_async_test(attrs: &TokenStream, input: TokenStream) -> Result<TokenStream> {
    if !attrs.is_empty() {
        return Err(Error::new(
            attrs.span(),
            "additional attributes are not supported",
        ));
    }

    let mut item_fn = parse2::<ItemFn>(input)?;

    let sig_span = item_fn.sig.span();

    item_fn
        .sig
        .asyncness
        .take()
        .ok_or_else(|| Error::new(sig_span, "Expected async function"))?;

    let output = build(item_fn);

    Ok(output)
}

fn build(mut fun_without_asyncness: ItemFn) -> TokenStream {
    let current_block = &fun_without_asyncness.block;

    fun_without_asyncness.block = parse_quote! {
        {
            crate::utils::RUNTIME.block_on(async #current_block);
        }
    };

    quote! {
        #[test]
        #fun_without_asyncness
    }
}

#[test]
fn wraps_async_test_in_runtime() {
    let out = attr_async_test(
        &quote! {},
        quote! {
            async fn works_when_doesnt_exist() {
                let name = fakeit::name::full();
                assert!(
                    !User::exists(
                        (&get_database()).into(),
                        user::filter! {
                            name: &name
                        },
                    )
                    .await
                    .unwrap()
                );
            }
        },
    )
    .unwrap();

    let expected = quote! {
        #[test]
        fn works_when_doesnt_exist() {
            crate::utils::RUNTIME.block_on(async {
                let name = fakeit::name::full();
                assert!(
                    !User::exists(
                        (&get_database()).into(),
                        user::filter! {
                            name: &name
                        },
                    )
                    .await
                    .unwrap()
                );
            });
        }
    };

    assert_eq!(out.to_string(), expected.to_string());
}
