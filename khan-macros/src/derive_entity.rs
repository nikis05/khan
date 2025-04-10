use crate::{
    prelude::*,
    utils::{build_fields_enum, extract_named_fields, extract_serde_rename, mongodb},
};

#[derive(FromAttributes)]
#[darling(attributes(entity))]
struct Attributes {
    #[darling(default)]
    projections: HashMap<Ident, PathList>,
    indexes: HashMap<Ident, IndexAttributes>,
}

#[derive(FromMeta)]
struct IndexAttributes {
    keys: HashMap<Ident, LitInt>,
    options: Expr,
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

    let indexes = attributes
        .indexes
        .into_iter()
        .map(|(name, index_attrs)| {
            let name = if name == "_" { None } else { Some(name) };

            let keys = index_attrs
                .keys
                .into_iter()
                .map(|(key, direction_lit)| {
                    if !fields.contains_key(&key) {
                        return Err(Error::new_spanned(key, "unknown field"));
                    }

                    let direction = match direction_lit.base10_parse::<i8>()? {
                        1 => IndexDirection::Pos,
                        -1 => IndexDirection::Neg,
                        _ => {
                            return Err(Error::new_spanned(
                                direction_lit,
                                "index direction must be `1` or `-1`",
                            ));
                        }
                    };

                    Ok((key, direction))
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
        &id_ty,
        &fields,
        &projections,
        &indexes,
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

struct IndexConfig {
    name: Option<Ident>,
    keys: HashMap<Ident, IndexDirection>,
    options: Expr,
}

enum IndexDirection {
    Pos,
    Neg,
}

fn build(
    vis: &Visibility,
    ident: &Ident,
    id_ty: &Type,
    fields: &HashMap<Ident, FieldConfig>,
    projections: &[ProjectionConfig],
    indexes: &[IndexConfig],
) -> TokenStream {
    let krate = krate();
    let mongodb = mongodb();

    let lowercase_entity = ident.to_string().to_snake_case();

    let mod_ident = Ident::new(&lowercase_entity, Span::call_site());

    let collection_name = LitStr::new(
        lowercase_entity
            .strip_suffix("_entity")
            .unwrap_or(&lowercase_entity),
        Span::call_site(),
    );

    let field_idents = fields.keys().collect_vec();

    let field_types = fields
        .values()
        .map(|field_config| &field_config.ty)
        .collect_vec();

    let filter_field_types = field_types.iter().map(|ty| {
        if let Type::Path(type_path) = ty {
            if type_path.qself.is_none() {
                if let Some(ident) = type_path.path.get_ident() {
                    if ident == "String" {
                        return parse_quote! { str };
                    }
                }
            }
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
                        .map_or_else(|| Cow::Owned(ident.to_string()), Cow::Borrowed),
                    Span::call_site(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let field_lits = field_lits_by_ident.values().collect_vec();

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
            let field_config = fields.get(ident).unwrap();

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

    quote! {
        #vis mod #mod_ident {
            use super::*;

            impl #krate::Entity for #ident {
                type Id = #id_ty;

                type Fields = Fields;

                const COLLECTION_NAME: &'static str = #collection_name;

                fn indexes() -> &'static [#mongodb::IndexModel] {
                    &[]
                }
            }

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
                fn to_document(&self) -> #mongodb::bson::Document {
                    let mut document = #mongodb::bson::doc! {};

                    #(
                        if let #krate::Field::Set(val) = &self.#field_idents {
                            #mongodb::bson::Document::insert(
                                &mut document,
                                #field_lits,
                                #krate::FilterOperator::to_document(val)
                            );
                        }
                    )*

                    document
                }
            }

            #[derive(::std::fmt::Debug, ::std::default::Default)]
            pub struct TypedUpdate {
                #(
                    pub #field_idents: #krate::Field<#field_types>
                ),*
            }

            impl #krate::Update<#ident> for TypedUpdate {
                fn to_document(&self) -> #mongodb::bson::Document {
                    let mut document = #mongodb::bson::doc! {};

                    #(
                        if let #krate::Field::Set(val) = &self.#field_idents {
                            #mongodb::bson::Document::insert(
                                &mut document,
                                #field_lits,
                                ::std::result::Result::unwrap(#mongodb::bson::to_bson(val)),
                            );
                        }
                    )*

                    document
                }
            }

            #update_apply_for_entity

            #( #projection_impls )*

            #fields_enum

            #[allow(unused_macros)]
            macro_rules! filter {
                ($( $input: tt )*) => {
                   #krate::construct_filter!(#mod_ident, $( $input )*)
                };
            }

            pub(crate) use filter;

            #[allow(unused_macros)]
            macro_rules! update {
                ($( $input: tt )*) => {
                   #krate::construct_update!(#mod_ident, $( $input )*)
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
