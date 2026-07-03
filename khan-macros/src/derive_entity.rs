#![allow(clippy::needless_continue)]

#[cfg(feature = "meta")]
use darling::FromMeta;
#[cfg(feature = "meta")]
use indexmap::IndexMap;

use crate::{
    prelude::*,
    utils::{build_fields_enum, extract_named_fields, extract_serde_rename, mongodb},
};

#[derive(FromAttributes)]
#[darling(attributes(entity))]
struct Attributes {
    #[darling(default)]
    collection: Option<String>,
    #[darling(default)]
    projections: HashMap<Ident, PathList>,
    #[cfg(feature = "meta")]
    #[darling(default)]
    indexes: HashMap<Ident, IndexAttributes>,
    #[cfg(feature = "meta")]
    #[darling(default)]
    untyped_indexes: UntypedIndexes,
    #[cfg(feature = "meta")]
    #[darling(default)]
    query_validation: Option<Expr>,
    #[cfg(all(feature = "meta", feature = "schema"))]
    #[darling(default)]
    skip_schema_validation: bool,
}

#[cfg(feature = "meta")]
#[derive(FromMeta)]
struct IndexAttributes {
    keys: IndexKeys,
    options: Option<Expr>,
}

#[cfg(feature = "meta")]
struct IndexKeys(IndexMap<Ident, i8>);

#[cfg(feature = "meta")]
#[derive(Default)]
struct UntypedIndexes(Punctuated<Expr, Token![,]>);

#[cfg(feature = "meta")]
impl FromMeta for UntypedIndexes {
    fn from_meta(item: &syn::Meta) -> darling::Result<Self> {
        let syn::Meta::List(list) = item else {
            return Err(
                darling::Error::custom("expected `untyped_indexes(index, ...)`").with_span(item),
            );
        };

        list.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)
            .map(Self)
            .map_err(Into::into)
    }
}

#[cfg(feature = "meta")]
impl FromMeta for IndexKeys {
    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let mut keys = IndexMap::with_capacity(items.len());

        for item in items {
            let darling::ast::NestedMeta::Meta(syn::Meta::NameValue(meta)) = item else {
                return Err(
                    darling::Error::custom("expected `field = 1` or `field = \"-1\"`")
                        .with_span(item),
                );
            };

            let ident = meta.path.get_ident().cloned().ok_or_else(|| {
                darling::Error::custom("expected field name").with_span(&meta.path)
            })?;

            let direction = match &meta.value {
                Expr::Lit(expr) => match &expr.lit {
                    syn::Lit::Int(lit) => lit.base10_parse::<i8>(),
                    syn::Lit::Str(lit) => lit
                        .value()
                        .parse::<i8>()
                        .map_err(|_| syn::Error::new(lit.span(), "expected `1` or `-1`")),
                    _ => Err(syn::Error::new(expr.span(), "expected `1` or `-1`")),
                },
                Expr::Unary(expr) if matches!(expr.op, syn::UnOp::Neg(_)) => match &*expr.expr {
                    Expr::Lit(expr) => match &expr.lit {
                        syn::Lit::Int(lit) => lit.base10_parse::<i8>().map(|val| -val),
                        _ => Err(syn::Error::new(expr.span(), "expected `1` or `-1`")),
                    },
                    _ => Err(syn::Error::new(expr.span(), "expected `1` or `-1`")),
                },
                expr => Err(syn::Error::new(expr.span(), "expected `1` or `-1`")),
            }
            .map_err(|err| darling::Error::custom(err).with_span(&meta.value))?;

            if keys.insert(ident.clone(), direction).is_some() {
                return Err(
                    darling::Error::duplicate_field(&ident.to_string()).with_span(&meta.path)
                );
            }
        }

        Ok(Self(keys))
    }
}

#[cfg(feature = "meta")]
enum IndexDirection {
    Pos,
    Neg,
}

pub fn derive_entity(item: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(item)?;

    let attributes = Attributes::from_attributes(&input.attrs)?;

    let (id_ty, fields) = {
        let fields_named = extract_named_fields(input.span(), input.data)?;

        let fields_span = fields_named.span();

        let mut id_ty = None;
        let mut fields = HashMap::new();

        for field in fields_named.named {
            let rename = extract_serde_rename(&field);

            if field.ident.as_ref().unwrap() == "id" {
                let missing_serde_attribute_err = || {
                    Error::new_spanned(&field, "id field must have `#[serde(rename = \"_id\")]`")
                };

                let Some(rename) = &rename else {
                    return Err(missing_serde_attribute_err());
                };

                if rename != "_id" {
                    return Err(missing_serde_attribute_err());
                }

                id_ty = Some(field.ty.clone());
            }

            fields.insert(
                field.ident.unwrap(),
                FieldConfig {
                    ty: field.ty,
                    rename,
                },
            );
        }

        let Some(id_ty) = id_ty else {
            return Err(Error::new(fields_span, "an entity must have an `id` field"));
        };

        (id_ty, fields)
    };

    let projections = attributes
        .projections
        .into_iter()
        .map(|(ident, projected_fields)| {
            let mut has_id = false;

            let mut projected_field_idents = vec![];

            for projected_field in projected_fields.iter() {
                let projected_field_ident = projected_field
                    .get_ident()
                    .cloned()
                    .ok_or_else(|| Error::new_spanned(projected_field, "expected ident"))?;
                if !fields.contains_key(&projected_field_ident) {
                    return Err(Error::new_spanned(projected_field_ident, "unknown field"));
                }

                if projected_field_ident == "id" {
                    has_id = true;
                }

                projected_field_idents.push(projected_field_ident);
            }

            Ok(ProjectionConfig {
                ident,
                has_id,
                fields: projected_field_idents,
            })
        })
        .try_collect::<_, Vec<_>, _>()?;

    #[cfg(feature = "meta")]
    let indexes = attributes
        .indexes
        .into_iter()
        .map(|(name, index_attrs)| {
            let name = if name == "__" { None } else { Some(name) };

            let keys = index_attrs
                .keys
                .0
                .into_iter()
                .map(|(key, direction_val)| {
                    let Some(field) = fields.get(&key) else {
                        return Err(Error::new_spanned(key, "unknown field"));
                    };

                    let direction = match direction_val {
                        1 => IndexDirection::Pos,
                        -1 => IndexDirection::Neg,
                        _ => {
                            return Err(Error::new_spanned(
                                direction_val,
                                "index direction must be `1` or `-1`",
                            ));
                        }
                    };

                    Ok((
                        field.rename.clone().unwrap_or_else(|| key.to_string()),
                        direction,
                    ))
                })
                .try_collect()?;

            Ok::<_, syn::Error>(IndexConfig {
                name,
                keys,
                options: index_attrs.options,
            })
        })
        .try_collect::<_, Vec<_>, _>()?;

    let output = build(
        &input.vis,
        &input.ident,
        attributes.collection.as_deref(),
        &id_ty,
        &fields,
        &projections,
        #[cfg(feature = "meta")]
        &indexes,
        #[cfg(feature = "meta")]
        attributes.untyped_indexes.0.iter(),
        #[cfg(feature = "meta")]
        attributes.query_validation.as_ref(),
        #[cfg(all(feature = "meta", feature = "schema"))]
        attributes.skip_schema_validation,
    );

    Ok(output)
}

struct FieldConfig {
    ty: Type,
    rename: Option<String>,
}

struct ProjectionConfig {
    ident: Ident,
    has_id: bool,
    fields: Vec<Ident>,
}

#[cfg(feature = "meta")]
struct IndexConfig {
    name: Option<Ident>,
    keys: IndexMap<String, IndexDirection>,
    options: Option<Expr>,
}

#[allow(clippy::extra_unused_lifetimes)]
#[allow(clippy::too_many_arguments)]
fn build<'a>(
    vis: &Visibility,
    ident: &Ident,
    collection_name: Option<&str>,
    id_ty: &Type,
    fields: &HashMap<Ident, FieldConfig>,
    projections: &[ProjectionConfig],
    #[cfg(feature = "meta")] indexes: &[IndexConfig],
    #[cfg(feature = "meta")] untyped_indexes: impl Iterator<Item = &'a Expr>,
    #[cfg(feature = "meta")] query_validation: Option<&Expr>,
    #[cfg(all(feature = "meta", feature = "schema"))] skip_schema_validation: bool,
) -> TokenStream {
    let krate = krate();
    let mongodb = mongodb();

    let lowercase_entity = ident.to_string().to_snake_case();

    let mod_ident = Ident::new(&lowercase_entity, Span::call_site());

    let collection_name = LitStr::new(
        collection_name.unwrap_or_else(|| {
            lowercase_entity
                .strip_suffix("_entity")
                .unwrap_or(&lowercase_entity)
        }),
        Span::call_site(),
    );

    let field_idents = fields.keys().collect_vec();

    let field_types = fields
        .values()
        .map(|field_config| &field_config.ty)
        .collect_vec();

    let filter_field_types = field_types.iter().map(|ty| {
        if let Type::Path(type_path) = ty
            && type_path.qself.is_none()
            && let Some(ident) = type_path.path.get_ident()
            && ident == "String"
        {
            return parse_quote! { str };
        }

        (*ty).to_owned()
    });

    let field_lits_by_ident = fields
        .iter()
        .map(|(field_ident, field_config)| {
            (
                field_ident,
                LitStr::new(
                    &field_config
                        .rename
                        .as_deref()
                        .map_or_else(|| Cow::Owned(field_ident.to_string()), Cow::Borrowed),
                    Span::call_site(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let field_lits = fields
        .keys()
        .map(|ident| field_lits_by_ident.get(ident).unwrap())
        .collect_vec();

    let update_apply_for_entity =
        build_update_apply(&krate, &mongodb, ident, field_idents.iter().copied());

    let projection_impls = projections.iter().map(|config| {
        let projection_ident = &config.ident;

        let projected_field_idents = &config.fields;

        let projected_field_lits = config.fields.iter().map(|field| field_lits_by_ident.get(field).unwrap());

        let selectable_with_id_impl = if config.has_id {
            quote! {
                impl #krate::SelectableWithId<#ident> for #projection_ident {
                    fn id(&self) -> <#ident as #krate::Entity>::Id {
                        self.id
                    }
                }
            }
        } else {
            quote! {}
        };

        let projection_fields = projected_field_idents.iter().map(|field_ident| {
            let field_config = fields.get(field_ident).unwrap();

            let field_ty = &field_config.ty;

            let rename_attr = if let Some(rename) = &field_config.rename {
                quote! { #[serde(rename = #rename)] }
            } else {
                quote! {}
            };

            quote! {
                #rename_attr
                pub #field_ident: #field_ty
            }
        });

        let update_apply_impl = build_update_apply(&krate, &mongodb, projection_ident, projected_field_idents.iter());

        quote! {
            #[derive(::std::fmt::Debug, ::serde::Serialize, ::serde::Deserialize)]
            pub struct #projection_ident {
                #( #projection_fields ),*
            }

            impl #krate::Selectable<#ident> for #projection_ident {
                const FIELDS: ::std::option::Option<&'static [&'static str]> = ::std::option::Option::Some(&[ #( #projected_field_lits ),* ]);
            }

            #selectable_with_id_impl

            #update_apply_impl

            impl ::std::convert::From<#ident> for #projection_ident {
                fn from(value: #ident) -> Self {
                    Self {
                        #(
                            #projected_field_idents: value.#projected_field_idents
                        ),*
                    }
                }
            }
        }
    });

    let fields_enum = build_fields_enum(field_idents.iter().copied(), field_lits.iter().copied());

    #[cfg(feature = "meta")]
    let indexes_impl = {
        let index_models = indexes
            .iter()
            .map(|index_config| {
                let keys = index_config.keys.iter().map(|(key, direction)| {
                    let key_lit = LitStr::new(&key.clone(), Span::call_site());
                    let direction_value = match direction {
                        IndexDirection::Pos => quote! { 1 },
                        IndexDirection::Neg => quote! { -1 },
                    };
                    quote! { #key_lit: #direction_value }
                });

                let options = if index_config.name.is_some() || index_config.options.is_some() {
                    let initial_options = if let Some(options) = &index_config.options {
                        quote! { #options }
                    } else {
                        quote! { #mongodb::options::IndexOptions::default() }
                    };

                    let assign_name = if let Some(name) = &index_config.name {
                        let name_lit = LitStr::new(&name.to_string(), Span::call_site());
                        quote! { options.name = Some(std::string::String::from(#name_lit)); }
                    } else {
                        quote! {}
                    };

                    quote! {
                        Some({
                            let mut options: #mongodb::options::IndexOptions = #initial_options;
                            #assign_name
                            options
                        })
                    }
                } else {
                    quote! { None }
                };

                quote! {
                    #mongodb::IndexModel::builder()
                      .keys(#mongodb::bson::doc! { #( #keys ),* })
                      .options(#options)
                      .build()
                }
            })
            .chain(untyped_indexes.map(|expr| quote! { #expr }));

        quote! {
            fn indexes() -> ::std::vec::Vec<#mongodb::IndexModel> {
                ::std::vec![ #( #index_models ),* ]
            }
        }
    };

    #[cfg(not(feature = "meta"))]
    let indexes_impl = quote! {};

    #[cfg(feature = "meta")]
    let query_validation_impl = {
        let query_validation = if let Some(query_validation) = query_validation {
            quote! {
                ::std::option::Option::Some(#query_validation)
            }
        } else {
            quote! { ::std::option::Option::None }
        };

        quote! {
            fn query_validation() -> ::std::option::Option<#mongodb::bson::Document> {
                #query_validation
            }
        }
    };

    #[cfg(not(feature = "meta"))]
    let query_validation_impl = quote! {};

    #[cfg(feature = "meta")]
    let submit_metadata = {
        #[cfg(feature = "schema")]
        let metadata_constructor = if skip_schema_validation {
            quote! { of_entity }
        } else {
            quote! { of_entity_with_schema }
        };

        #[cfg(not(feature = "schema"))]
        let metadata_constructor = quote! { of_entity };

        quote! {
            #krate::meta::__private__inventory_submit! {
                #krate::meta::__PRIVATE__EntityMetadataWrapper(#krate::meta::EntityMetadata::#metadata_constructor::<#ident>())
            }
        }
    };

    #[cfg(not(feature = "meta"))]
    let submit_metadata = quote! {};

    quote! {
        #vis mod #mod_ident {
            use super::*;

            impl #krate::Entity for #ident {
                type Id = #id_ty;

                type Fields = Fields;

                const COLLECTION_NAME: &'static str = #collection_name;

                #indexes_impl

                #query_validation_impl
            }

            #submit_metadata

            impl #krate::Selectable<Self> for #ident {
                const FIELDS: ::std::option::Option<&'static [&'static str]> = ::std::option::Option::None;
            }

            impl #krate::SelectableWithId<Self> for #ident {
                fn id(&self) -> <Self as #krate::Entity>::Id {
                    self.id
                }
            }

            #[derive(::std::fmt::Debug, ::std::default::Default)]
            pub struct TypedFilter<'a> {
                #(
                    pub #field_idents: #krate::Field<#krate::FilterOperator<'a, #filter_field_types>>
                ),*
            }

            impl #krate::Filter<#ident> for TypedFilter<'_> {
                fn to_document(&self) -> #mongodb::error::Result<#mongodb::bson::Document> {
                    let mut document = #mongodb::bson::Document::new();

                    #(
                        if let #krate::Field::Set(val) = &self.#field_idents {
                            #mongodb::bson::Document::insert(
                                &mut document,
                                #field_lits,
                                #krate::FilterOperator::to_document(val)?
                            );
                        }
                    )*

                    ::std::result::Result::Ok(document)
                }
            }

            #[derive(::std::fmt::Debug, ::std::default::Default)]
            pub struct TypedUpdate {
                #(
                    pub #field_idents: #krate::Field<#field_types>
                ),*
            }

            impl #krate::Update<#ident> for TypedUpdate {
                fn to_document(&self) -> #mongodb::error::Result<#mongodb::bson::Document> {
                    let mut inner_document = #mongodb::bson::Document::new();

                    #(
                        if let #krate::Field::Set(val) = &self.#field_idents {
                            #mongodb::bson::Document::insert(
                                &mut inner_document,
                                #field_lits,
                                #mongodb::bson::to_bson(val)
                                    .map_err(#mongodb::error::Error::custom)?,
                            );
                        }
                    )*

                    let mut document = #mongodb::bson::Document::new();
                    #mongodb::bson::Document::insert(
                        &mut document,
                        "$set",
                        inner_document,
                    );

                    ::std::result::Result::Ok(document)
                }
            }

            #update_apply_for_entity

            #( #projection_impls )*

            #fields_enum

            #[allow(unused_macros)]
            macro_rules! filter {
                ($( $input: tt )*) => {
                   #krate::__private__construct_filter!(#mod_ident, $( $input )*)
                };
            }

            pub(crate) use filter;

            #[allow(unused_macros)]
            macro_rules! update {
                ($( $input: tt )*) => {
                   #krate::__private__construct_update!(#mod_ident, $( $input )*)
                };
            }

            pub(crate) use update;
        }
    }
}

fn build_update_apply<'a>(
    krate: &TokenStream,
    mongodb: &TokenStream,
    apply_to: &Ident,
    field_idents: impl Iterator<Item = &'a Ident>,
) -> TokenStream {
    quote! {
        impl #krate::UpdateApply<#apply_to> for TypedUpdate {
            fn apply(self, projection: &mut #apply_to) -> #mongodb::error::Result<()> {
                #(
                    if let #krate::Field::Set(val) = self.#field_idents {
                        projection.#field_idents = val;
                    }
                )*

                ::std::result::Result::Ok(())
            }
        }
    }
}
