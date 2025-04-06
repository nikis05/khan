use crate::{
    prelude::*,
    utils::{build_fields_enum, extract_named_fields, extract_serde_rename, mongodb},
};

#[derive(FromAttributes)]
#[darling(attributes(entity))]
struct Attributes {
    #[darling(default)]
    projections: HashMap<Ident, PathList>,
}

pub fn derive_entity(item: TokenStream) -> Result<TokenStream> {
    let input = parse2::<DeriveInput>(item)?;

    let attributes = Attributes::from_attributes(&input.attrs)?;

    let (id_ty, fields) = {
        let fields_named = extract_named_fields(input.span(), input.data)?;

        let fields_span = fields_named.span();

        let mut id_ty = None;
        let mut fields = vec![];

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

            fields.push(FieldConfig {
                ident: field.ident.unwrap(),
                ty: field.ty,
                rename,
            });
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
                if !fields
                    .iter()
                    .any(|field| field.ident == projected_field_ident)
                {
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

    let output = build(&input.vis, &input.ident, &id_ty, &fields, &projections);

    Ok(output)
}

struct FieldConfig {
    ident: Ident,
    ty: Type,
    rename: Option<String>,
}

struct ProjectionConfig {
    ident: Ident,
    has_id: bool,
    fields: Vec<Ident>,
}

fn build(
    vis: &Visibility,
    ident: &Ident,
    id_ty: &Type,
    fields: &[FieldConfig],
    projections: &[ProjectionConfig],
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

    let field_idents = fields
        .iter()
        .map(|field_config| &field_config.ident)
        .collect_vec();

    let field_types = fields
        .iter()
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
        .map(|field| {
            (
                &field.ident,
                LitStr::new(
                    &field
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

        let projected_field_types = projected_field_idents.iter().map(|ident| {
            &fields
                .iter()
                .find(|field| field.ident == *ident)
                .unwrap()
                .ty
        });

        let projected_field_lits = config.fields.iter().map(|field| field_lits_by_ident.get(field).unwrap());

        let projection_with_id_impl = if config.has_id {
            quote! {
                impl #krate::ProjectionWithId<#ident> for #projection_ident {
                    fn id(&self) -> <#ident as #krate::Entity>::Id {
                        self.id
                    }
                }
            }
        } else {
            quote! {}
        };

        let update_apply_impl = build_update_apply(&krate, &mongodb, projection_ident, projected_field_idents.iter());

        quote! {
            #[derive(::std::fmt::Debug, ::serde::Serialize, ::serde::Deserialize)]
            pub struct #projection_ident {
                #(
                    pub #projected_field_idents: #projected_field_types
                ),*
            }

            impl #krate::Projection<#ident> for #projection_ident {
                const FIELDS: ::std::option::Option<&'static [&'static str]> = ::std::option::Option::Some(&[ #( #projected_field_lits ),* ]);
            }

            #projection_with_id_impl

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
            }

            impl #krate::Projection<Self> for #ident {
                const FIELDS: ::std::option::Option<&'static [&'static str]> = ::std::option::Option::None;
            }

            impl #krate::ProjectionWithId<Self> for #ident {
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
